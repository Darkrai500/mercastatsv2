#!/usr/bin/env node
"use strict";

const cp = require("node:child_process");
const fs = require("node:fs");
const path = require("node:path");

const RESET = "\x1b[0m";
const colors = {
  ocr: "\x1b[32m", // green
  backend: "\x1b[35m", // magenta
  frontend: "\x1b[36m", // cyan
  info: "\x1b[33m", // yellow
};

const projectRoot = __dirname;
const intelligenceDir = path.join(projectRoot, "intelligence-service");
const backendDir = path.join(projectRoot, "backend");
const frontendDir = path.join(projectRoot, "frontend");

const cliArgs = new Set(process.argv.slice(2));
const backendOnly = cliArgs.has("--backend-only");
const frontendOnly = cliArgs.has("--frontend-only");
const intelligenceOnly = cliArgs.has("--intelligence-only");
const noIntelligence = cliArgs.has("--no-intelligence");
const releaseMode = cliArgs.has("--release");

if (backendOnly && frontendOnly) {
  console.error("Cannot use --backend-only and --frontend-only together.");
  process.exit(1);
}

if (intelligenceOnly && noIntelligence) {
  console.error("Cannot use --intelligence-only and --no-intelligence together.");
  process.exit(1);
}

// Uvicorn no lee .env (el backend sí, con dotenvy): el lector de fotos del servicio Python
// recibe aquí sus variables. Solo esas, y si ya están en el entorno, manda el entorno.
const PHOTO_READER_VARS = ["OPENROUTER_API_KEY", "OCR_VISION_MODELS", "OCR_VISION_TIMEOUT"];

function readDotEnv(file) {
  const values = {};
  if (!fs.existsSync(file)) return values;
  for (const line of fs.readFileSync(file, "utf8").split(/\r?\n/)) {
    const match = line.match(/^\s*([A-Za-z_][A-Za-z0-9_]*)\s*=\s*(.*?)\s*$/);
    if (match) values[match[1]] = match[2].replace(/^(['"])(.*)\1$/, "$2");
  }
  return values;
}

const dotEnv = readDotEnv(path.join(projectRoot, ".env"));
const photoReaderEnv = Object.fromEntries(
  PHOTO_READER_VARS.filter((name) => process.env[name] === undefined && dotEnv[name]).map(
    (name) => [name, dotEnv[name]]
  )
);

const commands = [];

const runIntelligence = intelligenceOnly ? true : !noIntelligence;
const runBackend = intelligenceOnly ? false : !frontendOnly;
const runFrontend = intelligenceOnly ? false : !backendOnly;

if (runIntelligence) {
  // Prefiere el entorno virtual del proyecto (./run.sh lo crea): el Python global de
  // Homebrew/macOS no permite instalar paquetes (PEP 668).
  const venvPython = path.join(
    projectRoot,
    ".venv",
    process.platform === "win32" ? "Scripts/python.exe" : "bin/python"
  );
  const pythonCmd = fs.existsSync(venvPython)
    ? venvPython
    : process.platform === "win32"
      ? "python"
      : "python3";
  commands.push({
    name: "intelligence",
    cmd: pythonCmd,
    args: ["-m", "uvicorn", "main:app", "--host", "127.0.0.1", "--port", "8001", "--reload"],
    cwd: intelligenceDir,
    color: colors.ocr, // Reusing green color
    env: photoReaderEnv,
  });
}

if (runBackend) {
  const backendArgs = ["run"];
  if (releaseMode) {
    backendArgs.push("--release");
  }

  commands.push({
    name: "backend",
    cmd: "cargo",
    args: backendArgs,
    cwd: backendDir,
    color: colors.backend,
    // Consultas SQLx verificadas con los metadatos versionados en .sqlx/ (como en Docker),
    // sin necesitar la base de datos para compilar.
    env: { SQLX_OFFLINE: process.env.SQLX_OFFLINE ?? "true" },
  });
}

if (runFrontend) {
  commands.push({
    name: "frontend",
    cmd: "trunk",
    args: ["serve"],
    cwd: frontendDir,
    color: colors.frontend,
  });
}

if (commands.length === 0) {
  console.error("Nothing to run. Check command-line flags.");
  process.exit(1);
}

const running = new Map();
let shuttingDown = false;
let exitCode = 0;

function logInfo(message) {
  process.stdout.write(`${colors.info}[dev]${RESET} ${message}\n`);
}

function writeLine(prefixColor, prefixLabel, line) {
  process.stdout.write(`${prefixColor}[${prefixLabel}]${RESET} ${line}\n`);
}

function streamWithPrefix(stream, proc) {
  let buffer = "";
  const flush = () => {
    if (!buffer.length) {
      return;
    }
    writeLine(proc.color, proc.name, buffer);
    buffer = "";
  };

  stream.on("data", (chunk) => {
    buffer += chunk.toString();
    const parts = buffer.split(/\r?\n/);
    buffer = parts.pop() ?? "";
    for (const part of parts) {
      writeLine(proc.color, proc.name, part);
    }
  });

  stream.on("close", flush);
}

function shutdown(reason, desiredExitCode = 0) {
  if (shuttingDown) {
    return;
  }
  shuttingDown = true;
  if (exitCode === 0) {
    exitCode = desiredExitCode;
  }
  if (reason) {
    logInfo(`${reason} Stopping remaining processes...`);
  }

  for (const { child } of running.values()) {
    if (child.exitCode !== null || child.killed) {
      continue;
    }

    try {
      child.kill("SIGINT");
    } catch (err) {
      // Ignore
    }

    setTimeout(() => {
      if (child.exitCode === null && !child.killed) {
        try {
          child.kill("SIGTERM");
        } catch (err) {
          // Ignore
        }
      }
    }, 1000);

    if (process.platform === "win32") {
      setTimeout(() => {
        if (child.exitCode === null && !child.killed) {
          cp.spawn("taskkill", ["/PID", String(child.pid), "/T", "/F"], {
            stdio: "ignore",
          });
        }
      }, 4000);
    }
  }
}

function launch(proc) {
  logInfo(`Starting ${proc.name} (${proc.cmd} ${proc.args.join(" ")})`);

  const child = cp.spawn(proc.cmd, proc.args, {
    cwd: proc.cwd,
    env: { ...process.env, ...proc.env },
    stdio: ["inherit", "pipe", "pipe"],
  });

  running.set(proc.name, { child, proc });

  child.on("error", (err) => {
    writeLine(proc.color, proc.name, `failed to start: ${err.message}`);
    shutdown(`${proc.name} encountered an error.`, 1);
  });

  streamWithPrefix(child.stdout, proc);
  streamWithPrefix(child.stderr, proc);

  child.on("exit", (code, signal) => {
    const status =
      code !== null
        ? `exited with code ${code}`
        : `terminated by signal ${signal}`;
    writeLine(proc.color, proc.name, status);
    running.delete(proc.name);

    if (!shuttingDown) {
      const nextExitCode = code && code !== 0 ? code : 0;
      shutdown(`${proc.name} stopped.`, nextExitCode);
    } else if (code && exitCode === 0) {
      exitCode = code;
    }

    if (running.size === 0) {
      process.exit(exitCode);
    }
  });
}

process.on("SIGINT", () => {
  shutdown("Received SIGINT.", exitCode);
});

process.on("SIGTERM", () => {
  shutdown("Received SIGTERM.", exitCode);
});

for (const proc of commands) {
  launch(proc);
}

// In case commands finish immediately without spawning children.
if (running.size === 0) {
  process.exit(exitCode);
}
