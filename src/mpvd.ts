import pkg from "../package.json" with { type: "json" };
import { Command, program } from "commander";
import {
  getPause,
  getPlaylist,
  goToNext,
  goToPrev,
  playAtIndex,
  pushToPlaylist,
  send,
  setPause,
} from "./index.js";
import { spawnDaemon, killDaemon, printEnv, getDaemonPID } from "./env.js";

program.name("mpvd").description("MPV daemon control").version(pkg.version);

program
  .command("init")
  .description("Initialize mpv instance")
  .action(
    wrapped(async function () {
      const success = await spawnDaemon();
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
      console.log(pid);
    }),
  );

program
  .command("env")
  .description("Print environmemnt")
  .action(() => printEnv());

program
  .command("send")
  .description("Send IPC command")
  .arguments("<cmd...>")
  .action(
    wrapped(async function (args: string[]) {
      print(await send(...args));
    }),
  );

program
  .command("list")
  .alias("ls")
  .description("List current playlist")
  .option("-p, --plain")
  .action(
    wrapped(async function (_args, cmd: Command) {
      const { plain } = cmd.opts();
      const pause = await getPause();
      const playlist = await getPlaylist();
      const out = playlist
        .map(({ filename, current }, i) => {
          let id = String(i + 1).padStart(4, " ");
          if (process.stdout.isTTY) id = `\x1b[2m${id}\x1b[m`;
          const cursor = current ? (pause ? "-" : "*") : " ";
          let name = /[^\/]+$/.exec(filename)![0];
          if (current && process.stdout.isTTY && !plain)
            name = `\x1b[32m${name}\x1b[m`;
          if (plain) return name;
          return `${id} ${cursor} ${name}`;
        })
        .join("\n");
      print(out);
    }),
  );

program
  .command("play")
  .argument("[index]", "Playlist index to play at")
  .description("Start playback")
  .action(
    wrapped(async function ([arg] = []) {
      const index = arg && parseInt(arg);
      if (arg && isNaN(index)) throw new Error(`expected number, got "${arg}"`);
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
  .command("next")
  .description("Go to next file in playlist")
  .action(wrapped(goToNext));

program
  .command("prev")
  .alias("previous")
  .description("Go to previous file in playlist")
  .action(wrapped(goToPrev));

program.parse();

function print(value: any) {
  if (!value) return;
  if (typeof value === "string") console.log(value);
  else console.log(JSON.stringify(value));
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
