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
            Irradiance unit (wm2, lx, klx, fc).
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
        enableSoilMoisture = mkEnableOption "soil moisture sensors in the dashboard";
        enableLightning = mkEnableOption "lightning sensors in the dashboard";
        enableAirQuality = mkEnableOption "air quality sensors in the dashboard";
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
            ${optionalString cfg.debug "--debug"} \
            ${optionalString cfg.enableSoilMoisture "--enable-soil-moisture"} \
            ${optionalString cfg.enableLightning "--enable-lightning"} \
            ${optionalString cfg.enableAirQuality "--enable-air-quality"}
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

    services.grafana.provision.dashboards.settings.providers = mkIf cfg.enableGrafanaDashboard (
      let
        grafanaUnitMap = {
          temperature = { c = "celsius"; f = "fahrenheit"; k = "kelvin"; };
          wind = { kmh = "velocitykmh"; mph = "velocitymph"; ms = "velocityms"; knots = "velocityknot"; fps = "velocityfps"; };
          pressure = { hpa = "pressurehpa"; inhg = "pressureinHg"; mmhg = "pressurembar"; };
          rain = { mm = "lengthmm"; "in" = "lengthin"; };
          rainRate = { mm = "mm/h"; "in" = "in/h"; };
          distance = { km = "lengthkm"; mi = "lengthmi"; };
          irradiance = { wm2 = "Wm2"; lx = "lux"; klx = "klux"; fc = "fc"; };
        };
        gUnit = category: cfg_unit:
          grafanaUnitMap.${category}.${cfg_unit} or "${cfg_unit}";
        convertIrradiance = value:
          if cfg.irradianceUnit == "wm2" then value
          else if cfg.irradianceUnit == "lx" then value / 0.0079
          else if cfg.irradianceUnit == "klx" then value / 0.0079 / 1000.0
          else if cfg.irradianceUnit == "fc" then value * 6.345
          else value;
        irradianceStatThresholds = builtins.toJSON (map convertIrradiance [
          200.0
          400.0
          600.0
          800.0
          1000.0
        ]);
        irradianceHeatmapMin = builtins.toJSON (convertIrradiance 0.0);
        irradianceHeatmapMax = builtins.toJSON (convertIrradiance 1100.0);

        dashboardDir = pkgs.runCommand "ecowitt-grafana-dashboard" {
          nativeBuildInputs = [ pkgs.jq ];
          src = "${self}/grafana/EcowittWeatherStation.json";
        } ''
          mkdir -p $out
          jq '
            def set_unit(ids; unit):
              .panels |= map(
                if (.id as $id | ids | index($id)) then
                  .fieldConfig.defaults.unit = unit
                elif .panels then
                  .panels |= map(
                    if (.id as $id | ids | index($id)) then
                      .fieldConfig.defaults.unit = unit
                    else . end
                  )
                else . end
              );

            def set_threshold_values(ids; values):
              .panels |= map(
                if (.id as $id | ids | index($id)) then
                  .fieldConfig.defaults.thresholds.steps[1].value = values[0]
                  | .fieldConfig.defaults.thresholds.steps[2].value = values[1]
                  | .fieldConfig.defaults.thresholds.steps[3].value = values[2]
                  | .fieldConfig.defaults.thresholds.steps[4].value = values[3]
                  | .fieldConfig.defaults.thresholds.steps[5].value = values[4]
                elif .panels then
                  .panels |= map(
                    if (.id as $id | ids | index($id)) then
                      .fieldConfig.defaults.thresholds.steps[1].value = values[0]
                      | .fieldConfig.defaults.thresholds.steps[2].value = values[1]
                      | .fieldConfig.defaults.thresholds.steps[3].value = values[2]
                      | .fieldConfig.defaults.thresholds.steps[4].value = values[3]
                      | .fieldConfig.defaults.thresholds.steps[5].value = values[4]
                    else . end
                  )
                else . end
              );

            def set_heatmap_unit(ids; unit):
              .panels |= map(
                if (.id as $id | ids | index($id)) then
                  .fieldConfig.defaults.unit = unit
                  | .options.cellValues.unit = unit
                  | .options.yAxis.unit = unit
                elif .panels then
                  .panels |= map(
                    if (.id as $id | ids | index($id)) then
                      .fieldConfig.defaults.unit = unit
                      | .options.cellValues.unit = unit
                      | .options.yAxis.unit = unit
                    else . end
                  )
                else . end
              );

            def set_heatmap_scale(ids; min; max):
              .panels |= map(
                if (.id as $id | ids | index($id)) then
                  .options.color.min = min
                  | .options.color.max = max
                elif .panels then
                  .panels |= map(
                    if (.id as $id | ids | index($id)) then
                      .options.color.min = min
                      | .options.color.max = max
                    else . end
                  )
                else . end
              );

            def expand_row(id):
              .panels |= reduce .[] as $panel (
                [];
                if $panel.id == id then
                  . + [
                    ($panel
                      | .collapsed = false
                      | .panels = []
                    )
                  ] + ($panel.panels // [])
                else
                  . + [$panel]
                end
              );

            set_unit([36, 52, 53, 62]; "${gUnit "temperature" cfg.temperatureUnit}")
            | set_unit([38, 54, 55, 25]; "${gUnit "wind" cfg.windUnit}")
            | set_unit([37, 63]; "${gUnit "pressure" cfg.pressureUnit}")
            | set_unit([42]; "${gUnit "rain" cfg.rainUnit}")
            | set_unit([69]; "${gUnit "rain" cfg.rainUnit}")
            | set_unit([39]; "${gUnit "rainRate" cfg.rainUnit}")
            | set_unit([64, 67]; "${gUnit "distance" cfg.distanceUnit}")
            | set_unit([40]; "${gUnit "irradiance" cfg.irradianceUnit}")
            | set_threshold_values([40]; ${irradianceStatThresholds})
            | set_heatmap_unit([27]; "${gUnit "irradiance" cfg.irradianceUnit}")
            | set_heatmap_scale([27]; ${irradianceHeatmapMin}; ${irradianceHeatmapMax})
            ${optionalString cfg.enableSoilMoisture "| expand_row(61)"}
            ${optionalString cfg.enableLightning "| expand_row(68)"}
            ${optionalString cfg.enableAirQuality "| expand_row(44)"}
          ' "$src" > $out/EcowittWeatherStation.json
        '';
      in [
      {
        name = "${name}";
        options.path = dashboardDir;
        disableDeletion = true;
      }
    ]);
  };
}
