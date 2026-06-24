import React from "react";
import { Instance, render } from "ink";
import { Picker } from "./Picker.js";
import { Playlist } from "./Playlist.js";

let INK_INSTANCE: Instance | null = null;

function unmountInk() {
  INK_INSTANCE?.cleanup();
}

export function mountPicker({
  dirpath,
  unmount = unmountInk,
}: {
  dirpath?: string;
  unmount?: () => any;
} = {}) {
  unmountInk();
  INK_INSTANCE = render(<Picker dir={dirpath} unmount={unmount} />, {
    alternateScreen: true,
    exitOnCtrlC: false,
  });
}

export function mountPlaylist({
  unmount = unmountInk,
}: {
  unmount?: () => any;
} = {}) {
  unmountInk();
  INK_INSTANCE = render(<Playlist unmount={unmount} />, {
    alternateScreen: true,
    exitOnCtrlC: false,
  });
}
