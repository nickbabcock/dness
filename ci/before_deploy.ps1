# This script takes care of packaging the build artifacts that will go in the
# release zipfile

$SRC_DIR = $PWD.Path

Copy-Item "$SRC_DIR\target\$($Env:TARGET)\release\$($Env:CRATE_NAME).exe" "$SRC_DIR\$($Env:CRATE_NAME)-$($Env:APPVEYOR_REPO_TAG_NAME).exe"

Push-AppveyorArtifact "$SRC_DIR\$($Env:CRATE_NAME)-$($Env:APPVEYOR_REPO_TAG_NAME).exe"

Set-Location $SRC_DIR
