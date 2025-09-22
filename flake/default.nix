{ pkgs }:
{
  # select-source :: {
  #   name :: String,
  #   paths :: [ Path or FileSet ],
  # } -> Source
  #
  # Create a Source with the given name which includes the given
  # paths. Unlike `pkgs.symlinkJoin` these paths can be directories
  # OR files.
  select-source =
    let
      inherit (builtins) map;
      inherit (pkgs.lib.fileset) toSource unions;
      inherit (pkgs.lib.trivial) flip;
      inherit (pkgs) symlinkJoin;
    in
    { name, paths }:
    symlinkJoin {
      inherit name;
      paths = [
        (toSource {
          root = ../.;
          fileset = unions paths;
        })
      ];
    };

  # links-table :: (Name :: String) -> { relpath -> [Deriv or Path] } -> Derivation
  #
  # Create a derivation which maps relpath's to target paths or
  # derivations which come from a table (attrset names are relpaths).
  links-table =
    let
      inherit (pkgs.lib.attrsets) mapAttrsToList;
      inherit (pkgs) linkFarm;

      kv-to-entry = name: path: { inherit name path; };
    in
    name: table: linkFarm name (mapAttrsToList kv-to-entry table);
}
