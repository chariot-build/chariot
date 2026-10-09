---@meta

---@class ArchiveSource
---@field type '"archive"'
---@field url string
---@field checksum string
---@field kind '"tar"'

---@class GitSource
---@field type '"git"'
---@field url string
---@field revision string

--- @class LocalSource
--- @field type '"local"'
--- @field path string

---@alias SourceBase ArchiveSource|GitSource|LocalSource

---@alias Dependency string|SourceRef|PackageRef
---@alias Dependencies table<string|integer, Dependency>

---@class SourcePrepare
---@field script string
---@field dependencies Dependencies
---@field environment_variables table<string, any>

---@class SourceRef : userdata
---@class PackageRef : userdata

---@alias PackagePlatform
---| '"host"'
---| '"target"'

---@class PackageDef
---@field platform PackagePlatform
---@field name string
---@field version string
---@field revision integer
---@field dependencies Dependencies
---@field runtime_dependencies PackageRef[]
---@field environment_variables table<string, any>
---@field configure? string
---@field build? string
---@field install string

--- Table that exposes the Chariot API.
---@class ChariotAPI
---@field target_prefix string
---@field target_arch string
---@field options table<string, string>
chariot = {}

--- Reads a file's contents.
--- **NOTE**: The path will be relative to the project root,
--- which is the directory where the base config is.
---@param path string
---@return string
function chariot.read_file(path) end

--- Reads a directory's entries.
--- **NOTE**: The path will be relative to the project root,
--- which is the directory where the base config is.
---@param path string
---@return { filename: string, is_dir: boolean }[]
function chariot.list_dir(path) end

--- Concatenates two paths together in a platform correct way.
---@param path1 string
---@param path2 string
---@return string
function chariot.concat_paths(path1, path2) end

--- Registers a source and returns a handle to it.
---@param name string
---@param base SourceBase
---@param patches? string[]
---@param prepare? SourcePrepare
---@return SourceRef
function chariot.def_source(name, base, patches, prepare) end

--- Registers a package and returns a handle to it.
--- Errors if a package with the same name already exists for that platform.
---@param pkg PackageDef
---@return PackageRef
function chariot.def_package(pkg) end
