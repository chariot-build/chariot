--- Create an archive source table.
--- @param url string
--- @param checksum string
--- @param kind string?
--- @param compression string?
--- @return ArchiveSource
function Archive(url, checksum, kind, compression)
    if type(url) ~= "string" then
        error("archive url must be a string")
    end

    if type(checksum) ~= "string" then
        error("archive checksum must be a string")
    end

    if kind == nil then
        local urlParts = url:split(".")
        if #urlParts >= 2 then
            local part = urlParts[#urlParts - 1]
            if part == "tar" then
                kind = "tar"
            end
        end

        if kind == nil then
            error("could not infer archive kind from url")
        end
    end

    if compression == nil then
        if url:ends_with(".gz") then
            compression = "gz"
        elseif url:ends_with(".xz") then
            compression = "xz"
        elseif url:ends_with(".bz2") then
            compression = "bzip2"
        else
            error("could not infer archive compression from url")
        end
    end

    return {
        type = "archive",
        url = url,
        checksum = checksum,
        kind = kind,
        compression = compression
    }
end

--- Create a git source table.
--- @param url string
--- @param revision string
--- @return GitSource
function Git(url, revision)
    if type(url) ~= "string" then
        error("git url must be a string")
    end

    if type(revision) ~= "string" then
        error("git revision must be a string")
    end

    return {
        type = "git",
        url = url,
        revision = revision,
    }
end

--- Create a local source table.
--- @param path string
--- @return LocalSource
function Local(path)
    return {
        type = "local",
        path = path,
    }
end

--- Define a source and return a reference to it.
--- @param tbl { base: ArchiveSource|GitSource|LocalSource, patches?: string[], prepare: string, dependencies: Dependency[] }
--- @return SourceRef
function Source(tbl)
    local base = tbl["base"]
    if base == nil and #tbl >= 1 then
        base = tbl[1]
    end

    local patches = {}
    for _, patch in ipairs(tbl["patches"] or {}) do
        table.insert(patches, chariot.read_file(patch))
    end

    local deps = tbl["dependencies"]
    local script = tbl["prepare"]

    local prepare = nil
    if deps ~= nil and script ~= nil then
        prepare = { dependencies = deps, script = script }
    elseif deps ~= nil then
        warn("prepare skipped, `script` is defined but `dependencies` is not")
    elseif script ~= nil then
        warn("prepare skipped, `dependencies` is defined but `script` is not")
    end

    return chariot.def_source(base, patches, prepare)
end

local function pkg_helper(tbl)
    if tbl["dependencies"] == nil then
        tbl["dependencies"] = {}
    end

    if tbl["runtime_dependencies"] == nil then
        tbl["runtime_dependencies"] = {}
    end

    return chariot.def_package(tbl)
end

--- Define a target package and return a reference to it.
--- @param pkg { name: string, version: string, revision: number, dependencies?: Dependency[], runtime_dependencies?: PackageRef[], configure?: string, build?: string, install: string }
function Package(pkg)
    pkg["platform"] = "target"
    return pkg_helper(pkg)
end

--- Define a host package and return a reference to it.
--- @param tool { name: string, version: string, revision: number, dependencies?: Dependency[], runtime_dependencies?: PackageRef[], configure?: string, build?: string, install: string }
function Tool(tool)
    tool["platform"] = "host"
    return pkg_helper(tool)
end
