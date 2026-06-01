{ lib, pkgs ? import <nixpkgs> { } }:

let
  manifest = (pkgs.lib.importTOML ./Cargo.toml).package;
in
pkgs.rustPlatform.buildRustPackage rec {
  buildInputs = with pkgs; [
    lm_sensors
  ];

  pname = manifest.name;
  version = manifest.version;
  cargoLock.lockFile = ./Cargo.lock;
  src = pkgs.lib.cleanSource ./.;

  postInstall = ''
    mkdir -p $out/etc/udev/rules.d
    cp ${src}/sample/udev/*.rules $out/etc/udev/rules.d
    # Remove plugdev group in favor of uaccess tag
    sed -i 's/, GROUP="plugdev"//' $out/etc/udev/rules.d/*.rules
  '';

  meta = with lib; {
    mainProgram = pname;
    homepage = "https://github.com/Reikooters/antec-flux-pro-display";
    license = licenses.gpl3Only;
    platforms = platforms.linux;
  };
}
