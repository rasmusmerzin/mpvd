import { program } from "commander";
import pkg from "../package.json" with { type: "json" };
import { send } from "./index.js";

program.name("mpvctl").description("MPV daemon control").version(pkg.version);

program
  .command("send")
  .description("Send IPC command")
  .arguments("<cmd...>")
  .action(async function (args: string[]) {
    console.log(await send(...args));
  });

program.parse();
