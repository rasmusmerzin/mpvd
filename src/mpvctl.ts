import pkg from "../package.json" with { type: "json" };
import { Command, program } from "commander";
import { getPause, getPlaylist, playAtIndex, send, setPause } from "./index.js";

program.name("mpvctl").description("MPV daemon control").version(pkg.version);

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

program.parse();

function print(text: any) {
  if (text) console.log(text);
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
