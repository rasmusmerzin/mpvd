import { spawn } from "node:child_process";
import { readFile, rm, stat, writeFile } from "node:fs/promises";
import { homedir } from "node:os";

export const MPVD_SOCK =
  process.env.MPVD_SOCK ||
  (process.env.XDG_RUNTIME_DIR || process.env.HOME) + "/mpvd.sock";

export const MPVD_PID = MPVD_SOCK.replace(/[^\/]+$/, "mpvd.pid");

export async function printEnv() {
  console.log(`MPVD_SOCK=${MPVD_SOCK}`);
  console.log(`MPVD_PID=${MPVD_PID}`);
}

export async function startDaemon(): Promise<boolean> {
  const pidStat = await stat(MPVD_PID).catch(() => {});
  if (pidStat && pidStat.isFile()) return false;
  const child = spawn(
    "mpv",
    ["--idle", "--no-video", `--input-ipc-server=${MPVD_SOCK}`],
    {
      cwd: homedir(),
      detached: true,
      stdio: ["ignore", "ignore", "ignore"],
    },
  );
  child.unref();
  await writeFile(MPVD_PID, child.pid + "\n");
  return true;
}

export async function killDaemon(): Promise<boolean> {
  let success = true;
  const pid = await getDaemonPID();
  try {
    process.kill(pid!);
  } catch (e) {
    success = false;
  }
  try {
    await rm(MPVD_SOCK);
  } catch (e) {}
  try {
    await rm(MPVD_PID);
  } catch (e) {}
  return success;
}

export async function getDaemonPID(): Promise<number | null> {
  let pidText = "";
  try {
    pidText = await readFile(MPVD_PID, "utf-8");
  } catch (e) {
    return null;
  }
  const pid = parseInt(pidText.trim());
  return isNaN(pid) ? null : pid;
}
