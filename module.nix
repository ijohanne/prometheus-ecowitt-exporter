self:
{ config, lib, pkgs, ... }:
with lib;
let
  cfg = config.services.prometheus-ecowitt-exporter;
  name = "ecowitt";
  package = self.packages.${pkgs.stdenv.hostPlatform.system}.default;
in
{
  options.services.prometheus-ecowitt-exporter = with types; mkOption {
    type = types.submodule {
      options = {
        enable = mkEnableOption "the prometheus ${name} exporter";
        enableLocalScraping = mkEnableOption "scraping by local prometheus";
        enableGrafanaDashboard = mkEnableOption "provisioning of the Grafana dashboard";
        port = mkOption {
          type = types.port;
          default = 8088;
          description = ''
            Port to listen on.
          '';
        };
        listenAddress = mkOption {
          type = types.str;
          default = "0.0.0.0";
          description = ''
            Address to listen on.
          '';
        };
        temperatureUnit = mkOption {
          type = types.str;
          default = "c";
          description = ''
            Temperature unit (c, f, k).
          '';
        };
        pressureUnit = mkOption {
          type = types.str;
          default = "hpa";
          description = ''
            Pressure unit (hpa, inhg, mmhg).
          '';
        };
        windUnit = mkOption {
          type = types.str;
          default = "kmh";
          description = ''
            Wind speed unit (kmh, mph, ms, knots, fps).
          '';
        };
        rainUnit = mkOption {
          type = types.str;
          default = "mm";
          description = ''
            Rainfall unit (mm, in).
          '';
        };
        distanceUnit = mkOption {
          type = types.str;
          default = "km";
          description = ''
            Distance unit (km, mi).
          '';
        };
        irradianceUnit = mkOption {
          type = types.str;
          default = "wm2";
          description = ''
            Irradiance unit (wm2, lx, fc).
          '';
        };
        aqiStandard = mkOption {
          type = types.str;
          default = "epa";
          description = ''
            AQI standard (uk, epa, mep, nepm).
          '';
        };
        stationId = mkOption {
          type = types.str;
          default = "ecowitt";
          description = ''
            Station identifier.
          '';
        };
        outdoorLocation = mkOption {
          type = types.nullOr types.str;
          default = null;
          description = ''
            Label for outdoor sensor location.
          '';
        };
        indoorLocation = mkOption {
          type = types.nullOr types.str;
          default = null;
          description = ''
            Label for indoor sensor location.
          '';
        };
        tempLocations = mkOption {
          type = types.attrsOf types.str;
          default = { };
          description = ''
            Map of channel number to location label, e.g. { "1" = "Garden"; "2" = "Garage"; }.
          '';
        };
        forwardUrls = mkOption {
          type = types.listOf types.str;
          default = [ ];
          description = ''
            List of URLs to forward received data to (fire-and-forget, no TLS verification).
          '';
        };
        debug = mkOption {
          type = types.bool;
          default = false;
          description = ''
            Enable debug logging.
          '';
        };
        user = mkOption {
          type = types.str;
          default = "${name}-exporter";
          description = ''
            User name under which the ${name} exporter shall be run.
          '';
        };
        group = mkOption {
          type = types.str;
          default = "${name}-exporter";
          description = ''
            Group under which the ${name} exporter shall be run.
          '';
        };
      };
    };
    default = { };
  };
  config = mkIf cfg.enable {
    users.users."${cfg.user}" = {
      description = "Prometheus ${name} exporter service user";
      isSystemUser = true;
      group = "${cfg.group}";
    };
    users.groups."${cfg.group}" = { };

    systemd.services."prometheus-${name}-exporter" =
      let
        tempLocationArgs = concatStringsSep " " (
          mapAttrsToList (ch: loc: ''--temp${ch}-location "${loc}"'') cfg.tempLocations
        );
        forwardUrlArgs = concatStringsSep " " (
          map (url: ''--forward-url "${url}"'') cfg.forwardUrls
        );
        wrapper = pkgs.writeShellScript "prometheus-${name}-exporter" ''
          exec ${getBin package}/bin/prometheus-ecowitt-exporter \
            --listen-address ${cfg.listenAddress} \
            --listen-port ${toString cfg.port} \
            --temperature-unit ${cfg.temperatureUnit} \
            --pressure-unit ${cfg.pressureUnit} \
            --wind-unit ${cfg.windUnit} \
            --rain-unit ${cfg.rainUnit} \
            --distance-unit ${cfg.distanceUnit} \
            --irradiance-unit ${cfg.irradianceUnit} \
            --aqi-standard ${cfg.aqiStandard} \
            --station-id ${cfg.stationId} \
            ${optionalString (cfg.outdoorLocation != null) ''--outdoor-location "${cfg.outdoorLocation}"''} \
            ${optionalString (cfg.indoorLocation != null) ''--indoor-location "${cfg.indoorLocation}"''} \
            ${tempLocationArgs} \
            ${forwardUrlArgs} \
            ${optionalString cfg.debug "--debug"}
        '';
      in
      {
        wantedBy = [ "multi-user.target" ];
        after = [ "network.target" ];
        serviceConfig = {
          Restart = "always";
          PrivateTmp = true;
          WorkingDirectory = "/tmp";
          DynamicUser = false;
          User = cfg.user;
          Group = cfg.group;
          ExecStart = toString wrapper;
        };
      };

    services.prometheus.scrapeConfigs = mkIf cfg.enableLocalScraping [
      {
        job_name = "${name}";
        honor_labels = true;
        static_configs = [{
          targets = [ "127.0.0.1:${toString cfg.port}" ];
        }];
      }
    ];

    services.grafana.provision.dashboards.settings.providers = mkIf cfg.enableGrafanaDashboard [
      {
        name = "${name}";
        options.path = "${self}/grafana";
        disableDeletion = true;
      }
    ];
  };
}
