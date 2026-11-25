use std::{
    collections::HashMap,
    env,
    ffi::{CString, OsStr},
    fs::{File, create_dir_all, exists, metadata, remove_dir, remove_file, write},
    io::Write,
    os::fd::{AsFd, OwnedFd},
    panic,
    path::{Component, Path, PathBuf},
    process::exit,
};

use nix::{
    fcntl::OFlag,
    mount::{MntFlags, MsFlags, mount, umount2},
    poll::{PollFd, PollFlags, poll},
    sched::{CloneFlags, unshare},
    sys::wait::{WaitPidFlag, WaitStatus, waitpid},
    unistd::{
        ForkResult, Gid, Uid, chdir, dup2_stderr, dup2_stdin, dup2_stdout, execvp, fork, getegid, geteuid, pipe2, pivot_root, read, setgid, setuid,
    },
};

use crate::{Mount, MountKind, RuntimeError, StderrTarget};

enum StderrDest {
    Pipe(OwnedFd),
    MergeWithStdout,
    Null,
}

pub(super) fn runtime_execute_bare(
    rootfs_path: impl AsRef<Path>,
    uid: Uid,
    gid: Gid,
    cwd: impl AsRef<Path>,
    mounts: Vec<&Mount>,
    environment: HashMap<impl AsRef<OsStr>, impl AsRef<OsStr>>,
    network_isolation: bool,
    pid_isolation: bool,
    pipe_stdin: bool,
    mut stdout: Option<&mut dyn Write>,
    mut stderr: StderrTarget<'_>,
    args: Vec<impl AsRef<str>>,
) -> Result<i32, RuntimeError> {
    let stdout_pipe = match &stdout {
        Some(_) => Some(pipe2(OFlag::O_CLOEXEC).map_err(|errno| RuntimeError::Pipe { errno })?),
        None => None,
    };
    let stderr_pipe = match &stderr {
        StderrTarget::Capture(_) => Some(pipe2(OFlag::O_CLOEXEC).map_err(|errno| RuntimeError::Pipe { errno })?),
        StderrTarget::Discard | StderrTarget::Merge => None,
    };

    let fork_result = unsafe { fork() }.map_err(|errno| RuntimeError::Fork { errno })?;
    match fork_result {
        ForkResult::Parent { child: child_pid } => {
            let mut poll_fds = Vec::new();
            if let Some((read_fd, _)) = &stdout_pipe {
                poll_fds.push(PollFd::new(read_fd.as_fd(), PollFlags::POLLIN));
            }
            if let Some((read_fd, _)) = &stderr_pipe {
                poll_fds.push(PollFd::new(read_fd.as_fd(), PollFlags::POLLIN));
            }

            if poll_fds.is_empty() {
                return match waitpid(child_pid, None).map_err(|errno| RuntimeError::WaitPID { errno })? {
                    WaitStatus::Exited(_, code) => Ok(code),
                    status => Err(RuntimeError::InvalidWaitStatus { status }),
                };
            }

            let mut buffer = [0; 1024];
            loop {
                match waitpid(child_pid, Some(WaitPidFlag::WNOHANG)).map_err(|errno| RuntimeError::WaitPID { errno })? {
                    WaitStatus::StillAlive => {}
                    WaitStatus::Exited(_, code) => return Ok(code),
                    status => return Err(RuntimeError::InvalidWaitStatus { status }),
                }

                let n = poll(&mut poll_fds, 300_u16).map_err(|errno| RuntimeError::Poll { errno })?;
                if n == 0 {
                    continue;
                }

                let mut poll_idx = 0;

                if let Some((read_fd, _)) = &stdout_pipe {
                    let pollin = poll_fds[poll_idx].revents().map(|flags| flags.contains(PollFlags::POLLIN));
                    if matches!(pollin, Some(true)) {
                        let count = read(read_fd.as_fd(), &mut buffer).map_err(|errno| RuntimeError::Read { errno })?;
                        if count > 0
                            && let Some(writer) = stdout.as_deref_mut()
                        {
                            writer.write_all(&buffer[..count]).map_err(|err| RuntimeError::Write { source: err })?;
                            writer.flush().map_err(|err| RuntimeError::Write { source: err })?;
                        }
                    }
                    poll_idx += 1;
                }

                if let Some((read_fd, _)) = &stderr_pipe {
                    let pollin = poll_fds[poll_idx].revents().map(|flags| flags.contains(PollFlags::POLLIN));
                    if matches!(pollin, Some(true)) {
                        let count = read(read_fd.as_fd(), &mut buffer).map_err(|errno| RuntimeError::Read { errno })?;
                        if count > 0
                            && let StderrTarget::Capture(writer) = &mut stderr
                        {
                            writer.write_all(&buffer[..count]).map_err(|err| RuntimeError::Write { source: err })?;
                            writer.flush().map_err(|err| RuntimeError::Write { source: err })?;
                        }
                    }
                }
            }
        }
        ForkResult::Child => {
            let stdout_has_pipe = stdout_pipe.is_some();
            let stdout_fd = stdout_pipe.map(|(_, write_fd)| write_fd);
            let stderr_fd = match stderr_pipe {
                Some((_, write_fd)) => StderrDest::Pipe(write_fd),
                None if matches!(stderr, StderrTarget::Merge) && stdout_has_pipe => StderrDest::MergeWithStdout,
                None => StderrDest::Null,
            };

            child(
                rootfs_path,
                network_isolation,
                pid_isolation,
                uid,
                gid,
                cwd.as_ref(),
                mounts,
                environment,
                args.iter().map(|arg| arg.as_ref().to_string()).collect(),
                pipe_stdin,
                stdout_fd,
                stderr_fd,
            )
        }
    }
}

fn child(
    rootfs_path: impl AsRef<Path>,
    network_isolation: bool,
    pid_isolation: bool,
    uid: Uid,
    gid: Gid,
    cwd: &Path,
    mounts: Vec<&Mount>,
    environment: HashMap<impl AsRef<OsStr>, impl AsRef<OsStr>>,
    args: Vec<String>,
    pipe_stdin: bool,
    stdout_fd: Option<OwnedFd>,
    stderr_fd: StderrDest,
) -> ! {
    panic::set_hook(Box::new(|info| {
        eprintln!(
            "Chariot runtime (child process) panic `{}`",
            info.payload_as_str().unwrap_or("no message")
        );
        exit(1);
    }));

    let euid = geteuid();
    let egid = getegid();

    unshare(CloneFlags::CLONE_NEWUSER).expect("unshare user failed");

    write("/proc/self/setgroups", "deny").expect("setgroups write failed");
    write("/proc/self/gid_map", format!("{} {} 1", gid, egid)).expect("gid_map write failed");
    write("/proc/self/uid_map", format!("{} {} 1", uid, euid)).expect("uid_map write failed");

    setgid(gid).expect("setgid failed");
    setuid(uid).expect("setuid failed");

    if pid_isolation {
        unshare(CloneFlags::CLONE_NEWPID).expect("unshare pid failed");
    }

    let fork_result = unsafe { fork() }.expect("init process fork failed");
    match fork_result {
        ForkResult::Parent { child: child_pid } => {
            let status = waitpid(child_pid, None).expect("waitpid failed");

            if let WaitStatus::Exited(_, code) = status {
                exit(code);
            }

            panic!("waitpid returned invalid wait status");
        }
        ForkResult::Child => init(
            rootfs_path,
            network_isolation,
            cwd,
            mounts,
            environment,
            args,
            pipe_stdin,
            stdout_fd,
            stderr_fd,
        ),
    }
}

fn init(
    rootfs_path: impl AsRef<Path>,
    network_isolation: bool,
    cwd: &Path,
    mounts: Vec<&Mount>,
    environment: HashMap<impl AsRef<OsStr>, impl AsRef<OsStr>>,
    args: Vec<String>,
    pipe_stdin: bool,
    stdout_fd: Option<OwnedFd>,
    stderr_fd: StderrDest,
) -> ! {
    panic::set_hook(Box::new(|info| {
        eprintln!("Chariot runtime (init process) panic `{}`", info.payload_as_str().unwrap_or("no message"));
        exit(1);
    }));

    unshare(CloneFlags::CLONE_NEWNS).expect("unshare mounts failed");
    if network_isolation {
        unshare(CloneFlags::CLONE_NEWNET).expect("unshare network failed");
    }

    mount(None::<&str>, "/", None::<&str>, MsFlags::MS_REC | MsFlags::MS_PRIVATE, None::<&str>).expect("private mount of `/` failed");

    // Helpers
    let relative_rootfs_path = |path: &Path| -> PathBuf {
        let mut rootfs_relative_path = rootfs_path.as_ref().to_path_buf();

        for component in path.components() {
            match component {
                Component::Prefix(_) | Component::RootDir | Component::CurDir => {}
                Component::ParentDir => rootfs_relative_path.push(Component::ParentDir),
                Component::Normal(component) => rootfs_relative_path.push(component),
            }
        }

        rootfs_relative_path
    };

    let ensure_mountpoint = |mount: &Mount| {
        let is_file = match mount.kind {
            MountKind::Bind { is_file, .. } => is_file,
            _ => false,
        };

        let path = relative_rootfs_path(&mount.dest);
        if exists(&path).expect("prepare_mount failed: exists failed") {
            let meta = metadata(&path).expect("prepare_mount failed: metadata failed");
            if is_file && !meta.is_file() {
                remove_dir(&path).expect("prepare_mount failed: remove_dir failed");
            } else if !is_file && !meta.is_dir() {
                remove_file(&path).expect("prepare_mount failed: remove_file failed");
            } else {
                return;
            }
        }

        if is_file {
            if let Some(parent) = path.parent() {
                create_dir_all(parent).expect("prepare_mount failed: parent creation failed");
            }
            File::create(&path).expect("prepare_mount failed: file creation failed");
        } else {
            create_dir_all(&path).expect("prepare_mount failed: dir creation failed");
        }
    };

    let do_mount = |mount_config: &Mount| {
        let dest_path = relative_rootfs_path(&mount_config.dest);
        match &mount_config.kind {
            MountKind::Bind { from, read_only, .. } => {
                mount(Some(from), &dest_path, None::<&str>, MsFlags::MS_BIND | MsFlags::MS_REC, None::<&str>).expect("configured bind mount failed");
                if *read_only {
                    mount(
                        None::<&str>,
                        &dest_path,
                        None::<&str>,
                        MsFlags::MS_BIND | MsFlags::MS_REMOUNT | MsFlags::MS_RDONLY,
                        None::<&str>,
                    )
                    .expect("configured bind mount failed (readonly remount)");
                }
            }
            MountKind::Remount { readonly } => {
                let mut flags = MsFlags::MS_BIND | MsFlags::MS_REMOUNT;
                if *readonly {
                    flags |= MsFlags::MS_RDONLY;
                }

                mount(None::<&str>, &relative_rootfs_path(&mount_config.dest), None::<&str>, flags, None::<&str>).expect("configured remount failed");
            }
            MountKind::FS { fstype } => {
                mount(None::<&str>, &dest_path, Some(fstype.as_str()), MsFlags::empty(), None::<&str>).expect("configured fs mount failed");
            }
            MountKind::OverlayFS(overlay) => {
                mount(
                    None::<&str>,
                    &dest_path,
                    Some("overlay"),
                    MsFlags::empty(),
                    Some(overlay.data_string().as_os_str()),
                )
                .expect("configured overlayfs mount failed");
            }
        }
    };

    // Create mounts
    for mount in &mounts {
        ensure_mountpoint(mount);
        do_mount(mount);
    }

    // Enter rootfs
    chdir(rootfs_path.as_ref()).expect("rootfs chdir failed");
    pivot_root(".", ".").expect("pivot_root failed");
    umount2(".", MntFlags::MNT_DETACH).expect("old root unmount failed");
    chdir(&Path::new("/").join(cwd)).expect("cwd chdir failed");

    // Run program
    match unsafe { fork() }.expect("program fork failed") {
        ForkResult::Parent { child: child_pid } => {
            let status = waitpid(child_pid, None).expect("waitpid failed");

            if let WaitStatus::Exited(_, code) = status {
                exit(code);
            }

            panic!("waitpid returned invalid wait status");
        }
        ForkResult::Child => program(environment, args, pipe_stdin, stdout_fd, stderr_fd),
    };
}

fn program(
    environment: HashMap<impl AsRef<OsStr>, impl AsRef<OsStr>>,
    args: Vec<String>,
    pipe_stdin: bool,
    stdout_fd: Option<OwnedFd>,
    stderr_fd: StderrDest,
) -> ! {
    panic::set_hook(Box::new(|info| {
        eprintln!(
            "Chariot runtime (program process) panic `{}`",
            info.payload_as_str().unwrap_or("no message")
        );
        exit(1);
    }));

    if !pipe_stdin {
        let dev_null = File::options().read(true).open("/dev/null").expect("open /dev/null failed");
        dup2_stdin(dev_null.as_fd()).expect("dup2 stdin failed");
    }

    match &stdout_fd {
        Some(fd) => dup2_stdout(fd.as_fd()).expect("dup2 stdout failed"),
        None => {
            let dev_null = File::options().write(true).open("/dev/null").expect("open /dev/null failed");
            dup2_stdout(dev_null.as_fd()).expect("dup2 stdout failed");
        }
    }

    match stderr_fd {
        StderrDest::Pipe(fd) => dup2_stderr(fd.as_fd()).expect("dup2 stderr failed"),
        StderrDest::MergeWithStdout => {
            let fd = stdout_fd.as_ref().expect("stderr merge requested without a stdout pipe");
            dup2_stderr(fd.as_fd()).expect("dup2 stderr failed");
        }
        StderrDest::Null => {
            let dev_null = File::options().write(true).open("/dev/null").expect("open /dev/null failed");
            dup2_stderr(dev_null.as_fd()).expect("dup2 stderr failed");
        }
    }

    for name in env::vars().map(|(name, _)| name).collect::<Vec<_>>() {
        unsafe {
            env::remove_var(name);
        }
    }

    for (name, value) in environment.iter() {
        unsafe {
            env::set_var(name, value);
        }
    }

    let exec_result = execvp(
        &CString::new(args[0].as_str()).unwrap(),
        &args.iter().map(|a| CString::new(a.as_str()).unwrap()).collect::<Vec<_>>(),
    );

    panic!("error while executing program: {}", exec_result.unwrap_err());
}
