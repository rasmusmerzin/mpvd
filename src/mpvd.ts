import pkg from "../package.json" with { type: "json" };
import { Command, program } from "commander";
import {
  MPVD_PID,
  MPVD_SOCK,
  getDaemonPID,
  getDuration,
  getPause,
  getPlaylist,
  getPosition,
  getTime,
  getTimeString,
  goToNext,
  goToPrev,
  killDaemon,
  moveInPlaylist,
  playAtIndex,
  pushToPlaylist,
  removeFromPlaylist,
  send,
  setPause,
  startDaemon,
} from "./index.js";
import { mountPicker, mountPlaylist } from "./router.js";
import { printPlaylist } from "./playlist.js";

program.name("mpvd").description("MPV daemon control").version(pkg.version);

program
  .command("init")
  .alias("start")
  .description("Initialize mpv instance")
  .action(
    wrapped(async function () {
      const success = await startDaemon();
      process.exit(success ? 0 : 1);
    }),
  );

program
  .command("kill")
  .description("Kill mpv instance")
  .action(
    wrapped(async function () {
      const success = await killDaemon();
      process.exit(success ? 0 : 1);
    }),
  );

program
  .command("pid")
  .description("Print mpv instance PID")
  .action(
    wrapped(async function () {
      const pid = await getDaemonPID();
      if (pid == null) process.exit(1);
      print(pid);
    }),
  );

program
  .command("env")
  .description("Print environmemnt")
  .action(function () {
    print(`MPVD_SOCK=${MPVD_SOCK}\nMPVD_PID=${MPVD_PID}`);
  });

program
  .command("list")
  .alias("ls")
  .description("List current playlist")
  .option("-p, --plain", "Print without decorations")
  .option("-f, --full", "Print with absolute paths")
  .option("-i, --interactive", "Open playlist interactively")
  .action(
    wrapped(async function (_args, cmd: Command) {
      const { plain, full, interactive } = cmd.opts();
      if (interactive) mountPlaylist();
      else print(await printPlaylist({ plain, full }));
    }),
  );

program
  .command("position")
  .alias("pos")
  .description("Print playlist position of current track")
  .action(
    wrapped(async function () {
      print(await getPosition());
    }),
  );

program
  .command("time")
  .description("Print current track time position")
  .option("-s, --seconds", "Print seconds")
  .option("-d, --duration", "Print duration")
  .action(
    wrapped(async function (_args, cmd: Command) {
      const { seconds, duration } = cmd.opts();
      if (!seconds && !duration) print(await getTimeString());
      else if (seconds && !duration) print(await getTime());
      else if (!seconds && duration) print(await getDuration());
      else if (seconds && duration)
        print(`${await getTime()}/${await getDuration()}`);
    }),
  );

program
  .command("state")
  .description("Print playing/paused state")
  .action(
    wrapped(async function () {
      const pause = await getPause();
      print(pause ? "paused" : "playing");
    }),
  );

program
  .command("current")
  .description("Print current track")
  .action(
    wrapped(async function () {
      const playlist = await getPlaylist();
      print(playlist.find((item) => item.current)?.filename);
    }),
  );

program
  .command("play")
  .argument("[index]", "Playlist index to play at")
  .description("Start playback")
  .action(
    wrapped(async function ([arg] = []) {
      const index = safeParseInt(arg);
      if (index) await playAtIndex(index);
      await setPause(false);
    }),
  );

program
  .command("stop")
  .description("Pause playback")
  .action(
    wrapped(async function () {
      await setPause(true);
    }),
  );

program
  .command("push")
  .arguments("<files...>")
  .description("Push files to playlist")
  .action(
    wrapped(async function (files: string[]) {
      for (const file of files) await pushToPlaylist(file);
    }),
  );

program
  .command("pick")
  .description("Pick files to playlist")
  .argument("[dirpath]", "Directory path", "~/Music")
  .action(wrapped((dirpath) => mountPicker({ dirpath })));

program
  .command("next")
  .description("Go to next file in playlist")
  .action(wrapped(goToNext));

program
  .command("prev")
  .alias("previous")
  .description("Go to previous file in playlist")
  .action(wrapped(goToPrev));

program
  .command("move")
  .alias("mv")
  .argument("<from>")
  .argument("<to>")
  .description("Move file in playlist")
  .action(
    wrapped(async function (from: string, to: string) {
      await moveInPlaylist(safeParseInt(from)!, safeParseInt(to)!);
    }),
  );

program
  .command("remove")
  .alias("rm")
  .argument("<index>")
  .description("Remove file at index from playlist")
  .action(
    wrapped(async function (index: string) {
      await removeFromPlaylist(safeParseInt(index)!);
    }),
  );

program
  .command("send")
  .arguments("<cmd...>")
  .description("Send IPC command")
  .action(
    wrapped(async function (args: string[]) {
      print(await send(...args));
    }),
  );

program.parse();

function safeParseInt(arg?: string) {
  if (!arg) return;
  const int = parseInt(arg);
  if (arg && isNaN(int)) throw new Error(`expected number, got "${arg}"`);
  return int;
}

function print(value: any) {
  if (!value) return;
  if (typeof value === "string") console.log(value);
  else if (typeof value === "object") console.log(JSON.stringify(value));
  else console.log(String(value));
}

function wrapped<T, A extends any[]>(
  this: T,
  fn: (this: T, ...args: A) => any,
) {
  return async (...args: A) => {
    try {
      await fn.call(this, ...args);
    } catch (err) {
      console.error(String(err));
      process.exit(1);
    }
  };
}
