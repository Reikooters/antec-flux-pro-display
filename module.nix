{ config, lib, pkgs, ... }:

let
  cfg = config.services.antec-flux-pro-display;
  toConf = lib.generators.toKeyValue {};
  confAttrs = lib.filterAttrs (_: v: v != null) cfg.settings;
in {
  options.services.antec-flux-pro-display = {
    enable = lib.mkEnableOption "Antec Flux Pro display";

    package = lib.mkOption {
      type = lib.types.package;
      default = pkgs.callPackage ./package.nix {};
      defaultText = lib.literalExpression "pkgs.callPackage ./package.nix {}";
      description = "Package providing antec-flux-pro-display.";
    };

    settings = lib.mkOption {
      description = "Settings written to /etc/antec-flux-pro-display/config.conf";
      type = lib.types.submodule {
        options = {
          cpu_device = lib.mkOption {
            type = lib.types.str;
            description = "CPU temperature device name.";
            example = "k10temp";
          };
          cpu_temp_type = lib.mkOption {
            type = lib.types.str;
            description = "CPU temperature sensor label.";
            example = "tctl";
          };
          cpu_vendor_id = lib.mkOption {
            type = lib.types.nullOr lib.types.str;
            description = "**Optional**, use it in addition to the name if you have two devices with the same name";
            default = null;
            example = "1022";
          };
          cpu_device_id = lib.mkOption {
            type = lib.types.nullOr lib.types.str;
            description = "**Optional**, use it in addition to the name if you have two devices with the same name";
            default = null;
            example = "14e3";
          };

          gpu_device = lib.mkOption {
            type = lib.types.str;
            description = "GPU temperature device name.";
            example = "amdgpu";
          };
          gpu_temp_type = lib.mkOption {
            type = lib.types.str;
            description = "GPU temperature sensor label.";
            example = "edge";
          };
          gpu_vendor_id = lib.mkOption {
            type = lib.types.nullOr lib.types.str;
            description = "**Optional**, use it in addition to the name if you have two devices with the same name.";
            default = null;
            example = "1002";
          };
          gpu_device_id = lib.mkOption {
            type = lib.types.nullOr lib.types.str;
            description = "**Optional**, use it in addition to the name if you have two devices with the same name.";
            default = null;
            example = "7550";
          };

          update_interval = lib.mkOption {
            type = lib.types.int;
            description = "Update interval in milliseconds.";
            default = 1000;
          };
        };
      };

      default = {};
    };
  };

  config = lib.mkIf cfg.enable {
    services.udev.packages = [ cfg.package ];

    environment.etc."antec-flux-pro-display/config.conf".text = toConf confAttrs;

    systemd.services.antec-flux-pro-display = {
      description = "Antec Flux Pro Display Service";
      wantedBy = [ "multi-user.target" ];
      after = [ "systemd-udevd.service" ];
      restartTriggers = [ config.environment.etc."antec-flux-pro-display/config.conf".source ];
      serviceConfig = {
        Type = "simple";
        ExecStart = lib.getExe cfg.package;
        Restart = "always";
        RestartSec = 5;
        ProtectSystem = "strict";
        ProtectHome = true;
        PrivateTmp = true;
        NoNewPrivileges = true;
      };
    };
  };
}
