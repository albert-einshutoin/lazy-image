const fs = require("fs");
const path = require("path");

const root = path.resolve(__dirname, "..");
const pkgPath = path.join(root, "package.json");
const cargoPath = path.join(root, "Cargo.toml");

function readJson(filePath) {
  return JSON.parse(fs.readFileSync(filePath, "utf8"));
}

function writeJson(filePath, data) {
  const content = JSON.stringify(data, null, 2) + "\n";
  fs.writeFileSync(filePath, content);
}

function syncPackageJsonVersion(pkg) {
  if (!pkg.optionalDependencies) return false;
  let updated = false;
  for (const name of Object.keys(pkg.optionalDependencies)) {
    if (pkg.optionalDependencies[name] !== pkg.version) {
      pkg.optionalDependencies[name] = pkg.version;
      updated = true;
    }
  }
  return updated;
}

function syncCargoVersion(version) {
  const cargo = fs.readFileSync(cargoPath, "utf8");
  const next = cargo.replace(
    /^version\s*=\s*"[0-9A-Za-z.+-]+"/m,
    `version = "${version}"`
  );
  if (cargo === next) return false;
  fs.writeFileSync(cargoPath, next);
  return true;
}

function main() {
  const pkg = readJson(pkgPath);
  const version = pkg.version;

  const pkgChanged = syncPackageJsonVersion(pkg);
  if (pkgChanged) {
    writeJson(pkgPath, pkg);
  }

  const cargoChanged = syncCargoVersion(version);

  if (!pkgChanged && !cargoChanged) {
    console.log("Versions already synchronized:", version);
  } else {
    console.log(
      `Synchronized versions to ${version}` +
        `${pkgChanged ? " (package.json optionalDependencies)" : ""}` +
        `${cargoChanged ? " (Cargo.toml)" : ""}`
    );
  }
}

main();

