#!/usr/bin/env node
"use strict";

const cp = require("node:child_process");
const path = require("node:path");

const RESET = "\x1b[0m";
const colors = {
  ocr: "\x1b[32m", // green
  backend: "\x1b[35m", // magenta
  frontend: "\x1b[36m", // cyan
  info: "\x1b[33m", // yellow
};

const projectRoot = __dirname;
const ocrDir = path.join(projectRoot, "ocr-service");
const backendDir = path.join(projectRoot, "backend");
const frontendDir = path.join(projectRoot, "frontend");

const cliArgs = new Set(process.argv.slice(2));
const backendOnly = cliArgs.has("--backend-only");
const frontendOnly = cliArgs.has("--frontend-only");
const ocrOnly = cliArgs.has("--ocr-only");
const noOcr = cliArgs.has("--no-ocr");
const releaseMode = cliArgs.has("--release");

if (backendOnly && frontendOnly) {
  console.error("Cannot use --backend-only and --frontend-only together.");
  process.exit(1);
}

if (ocrOnly && noOcr) {
  console.error("Cannot use --ocr-only and --no-ocr together.");
  process.exit(1);
}

const commands = [];

const runOcr = ocrOnly ? true : !noOcr;
const runBackend = ocrOnly ? false : !frontendOnly;
const runFrontend = ocrOnly ? false : !backendOnly;

if (runOcr) {
  const pythonCmd = process.platform === "win32" ? "python" : "python3";
  commands.push({
    name: "ocr",
    cmd: pythonCmd,
    args: ["-m", "uvicorn", "src.main:app", "--host", "127.0.0.1", "--port", "9000", "--reload"],
    cwd: ocrDir,
    color: colors.ocr,
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
    env: process.env,
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
