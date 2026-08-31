# Grok OSS workers on a mail/web host (surmount-1).
#
# Import this fragment from the host. Complements sibling surmount-server
# modules/grok-oss.nix (user grok, MemoryMax, no boot TUI). This file does
# not replace that module.
#
# Does not start a second Nix daemon. Uses the host's existing daemon.
# Requires a non-empty systemd MemoryMax. Does not disable surmount-scram.
# Workers are killable; scram is not. Does not start the grok-oss TUI on boot.
# Optional instance working directories stay at sshd class (do not set Nice=).

{
  config,
  lib,
  ...
}:
let
  cfg = config.grokOssWorkers;
  inherit (lib) mkEnableOption mkIf mkOption types;
in
{
  options.grokOssWorkers = {
    enable = mkEnableOption "memory-capped grok-oss workers (no boot TUI)";

    memoryMax = mkOption {
      type = types.str;
      default = "";
      example = "4G";
      description = ''
        systemd MemoryMax for the grok-oss-workers slice. Required non-empty
        when enable is true. Empty refuses. Host-local sets the real budget.
        This is not a published guest RAM size.
      '';
    };

    user = mkOption {
      type = types.str;
      default = "grok";
      description = "Unprivileged owner of worker working directories. Not root.";
    };

    instanceCwds = mkOption {
      type = types.listOf types.str;
      default = [ ];
      description = ''
        Optional absolute working directories for grok-oss worker instances.
        Directories are created. Do not raise Nice above sshd class. Do not
        start the grok-oss TUI on boot from these paths.
      '';
    };
  };

  config = mkIf cfg.enable {
    assertions = [
      {
        assertion = cfg.memoryMax != "";
        message = "grokOssWorkers.memoryMax must be a non-empty systemd MemoryMax string when enable is true.";
      }
      {
        assertion = cfg.user != "root";
        message = "grokOssWorkers.user must not be root (do not cap the whole machine).";
      }
      {
        assertion = builtins.all (p: builtins.substring 0 1 p == "/") cfg.instanceCwds;
        message = "grokOssWorkers.instanceCwds entries must be absolute paths.";
      }
    ];

    systemd.slices.grok-oss-workers = {
      description = "Grok OSS workers (hard memory cap; killable; scram is not)";
      sliceConfig = {
        MemoryAccounting = true;
        MemoryMax = cfg.memoryMax;
      };
    };

    systemd.tmpfiles.rules = map (d: "d ${d} 0750 ${cfg.user} ${cfg.user} -") cfg.instanceCwds;
  };
}
