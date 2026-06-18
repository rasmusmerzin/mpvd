import { program } from "commander";
import pkg from "../package.json" with { type: "json" };
import { list, send } from "./index.js";

program.name("mpvctl").description("MPV daemon control").version(pkg.version);

program
  .command("send")
  .description("Send IPC command")
  .arguments("<cmd...>")
  .action(async function (args: string[]) {
    console.log(await send(...args));
  });

program
  .command("list")
  .description("List current playlist")
  .option("-p, --plain")
  .action(async function () {
    const { plain } = this.opts();
    console.log(await list({ plain }));
  });

program.parse();
