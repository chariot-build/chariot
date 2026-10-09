--- Create an archive source table.
--- @param url string
--- @param checksum string
--- @param kind string?
--- @return ArchiveSource
function Archive(url, checksum, kind)
    if type(url) ~= "string" then
        error("archive url must be a string")
    end

    if type(checksum) ~= "string" then
        error("archive checksum must be a string")
    end

    if kind == nil then
        local path = url:gsub("[?#].*$", ""):lower()

        if path:match("%.tar%.%w+$") or path:match("%.tgz$") or path:match("%.tar$") then
            kind = "tar"
        elseif path:match("%.zip$") then
            kind = "zip"
        else
            error("could not infer archive kind from url: " .. url)
        end
    end

    return {
        type = "archive",
        url = url,
        checksum = checksum,
        kind = kind,
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
    if type(path) ~= "string" then
        error("local path must be a string")
    end

    return {
        type = "local",
        path = path
    }
end

--- Define a source and return a reference to it.
--- @param tbl { name: string, base: ArchiveSource|GitSource, patches?: string[], prepare?: string, dependencies?: Dependency[], env?: table<string, string | number> }
--- @return SourceRef
function Source(tbl)
    local name = tbl["name"]
    if name == nil then
        for i = 1, #tbl do
            if type(tbl[i]) == "string" then
                name = tbl[i]
                break
            end
        end
    end
    if type(name) ~= "string" then
        error("name must be a string")
    end

    local base = tbl["base"]
    if base == nil then
        for i = 1, #tbl do
            if type(tbl[i]) == "table" then
                base = tbl[i]
                break
            end
        end
    end
    if base == nil then
        error("no source base provided")
    end

    local patches = {}
    for _, patch in ipairs(tbl["patches"] or {}) do
        table.insert(patches, chariot.read_file(patch))
    end

    local script = tbl["prepare"]
    local deps = tbl["dependencies"]
    local env = tbl["env"]

    local prepare = nil
    if script ~= nil then
        prepare = { dependencies = deps or {}, environment_variables = env or {}, script = script }
    else
        if deps ~= nil then
            warn("prepare skipped, `dependencies` is defined but `script` is not")
        end

        if env ~= nil then
            warn("prepare skipped, `env` is defined but `script` is not")
        end
    end

    return chariot.def_source(name, base, patches, prepare)
end

local function pkg_helper(platform, tbl)
    if platform ~= "target" and platform ~= "host" then
        error("wux fucked up, invalid platform `" .. platform .. "`")
    end

    local pkg = {}

    pkg["platform"] = platform

    local keytypes <const> = {
        name = { true, "string" },
        version = { true, "string" },
        revision = { true, "number" },
        env = { false, "table" },
        source = { false, "userdata" },
        dependencies = { false, "table" },
        runtime_dependencies = { false, "table" },
        configure = { false, "string" },
        build = { false, "string" },
        install = { true, "string" },
    }

    for key, types in pairs(keytypes) do
        if (types[1] and type(tbl[key]) ~= types[2]) and type(tbl[key]) ~= "nil" then
            error(key .. " must be a " .. types[2])
        end

        pkg[key] = tbl[key]
    end

    pkg["environment_variables"] = {}
    if type(tbl["env"]) ~= "nil" then
        for k, v in pairs(tbl["env"]) do
            if type(k) ~= "string" then
                error("env must only contain string keys")
            end

            if type(v) ~= "string" and type(v) ~= "number" then
                error("env must only contain string or number values")
            end

            pkg["environment_variables"][k] = v
        end
    end

    pkg["dependencies"] = {}
    if type(tbl["dependencies"]) ~= "nil" then
        for k, v in pairs(tbl["dependencies"]) do
            if type(k) ~= "number" then
                error("dependencies must only contain numeric keys")
            end

            pkg["dependencies"][k] = v
        end
    end

    pkg["runtime_dependencies"] = {}
    if type(tbl["runtime_dependencies"]) ~= "nil" then
        for k, v in pairs(tbl["runtime_dependencies"]) do
            if type(k) ~= "number" then
                error("runtime_dependencies must only contain numeric keys")
            end

            pkg["runtime_dependencies"][k] = v
        end
    end

    return chariot.def_package(pkg)
end

--- Define a target package and return a reference to it.
--- @param pkg { name: string, version: string, revision: number, source: SourceRef?, dependencies?: Dependency[], runtime_dependencies?: PackageRef[], env?: table<string, string | number>, configure?: string, build?: string, install: string }
function Package(pkg)
    return pkg_helper("target", pkg)
end

--- Define a host package and return a reference to it.
--- @param tool { name: string, version: string, revision: number, source: SourceRef?, dependencies?: Dependency[], runtime_dependencies?: PackageRef[], env?: table<string, string | number>, configure?: string, build?: string, install: string }
function Tool(tool)
    return pkg_helper("host", tool)
end
