import { homedir } from "node:os";
import { readFile, rm, writeFile } from "node:fs/promises";
import { spawn } from "node:child_process";
import { MPVD_SOCK, MPVD_PID } from "./index.js";

export async function startDaemon(): Promise<boolean> {
  const pid = await getDaemonPID();
  if (pid !== null) return false;
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
