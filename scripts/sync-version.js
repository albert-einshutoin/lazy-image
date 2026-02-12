const fs = require("fs");
const path = require("path");

const root = path.resolve(__dirname, "..");
const pkgPath = path.join(root, "package.json");
const cargoPath = path.join(root, "Cargo.toml");
const lockPath = path.join(root, "package-lock.json");

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

function syncPackageLock(pkg) {
  if (!fs.existsSync(lockPath)) return false;
  const lock = readJson(lockPath);
  let changed = false;

  if (lock.version && lock.version !== pkg.version) {
    lock.version = pkg.version;
    changed = true;
  }

  if (lock.packages && lock.packages[""]) {
    const rootPkg = lock.packages[""];
    if (rootPkg.version !== pkg.version) {
      rootPkg.version = pkg.version;
      changed = true;
    }
    if (rootPkg.optionalDependencies && pkg.optionalDependencies) {
      for (const name of Object.keys(pkg.optionalDependencies)) {
        if (rootPkg.optionalDependencies[name] !== pkg.version) {
          rootPkg.optionalDependencies[name] = pkg.version;
          changed = true;
        }
      }
    }
  }

  if (lock.packages && pkg.optionalDependencies) {
    for (const [key, entry] of Object.entries(lock.packages)) {
      if (!entry) continue;
      const entryName = entry.name
        ? entry.name
        : key.startsWith("node_modules/")
          ? key.slice("node_modules/".length)
          : key;
      if (pkg.optionalDependencies[entryName] && entry.version !== pkg.version) {
        entry.version = pkg.version;
        if (entry.resolved) {
          entry.resolved = entry.resolved.replace(
            /-[0-9]+\.[0-9]+\.[0-9]+\.tgz$/,
            `-${pkg.version}.tgz`
          );
          // Clear integrity so next `npm install` regenerates it for the new tarball.
          // Otherwise npm ci can fail with integrity mismatch after publish.
          if (entry.integrity) {
            delete entry.integrity;
          }
        }
        changed = true;
      }
    }
  }

  if (changed) {
    writeJson(lockPath, lock);
  }
  return changed;
}

function main() {
  const pkg = readJson(pkgPath);
  const version = pkg.version;

  const pkgChanged = syncPackageJsonVersion(pkg);
  if (pkgChanged) {
    writeJson(pkgPath, pkg);
  }

  const lockChanged = syncPackageLock(pkg);
  const cargoChanged = syncCargoVersion(version);

  if (!pkgChanged && !cargoChanged && !lockChanged) {
    console.log("Versions already synchronized:", version);
  } else {
    console.log(
      `Synchronized versions to ${version}` +
        `${pkgChanged ? " (package.json optionalDependencies)" : ""}` +
        `${lockChanged ? " (package-lock.json)" : ""}` +
        `${cargoChanged ? " (Cargo.toml)" : ""}`
    );
  }
}

main();

