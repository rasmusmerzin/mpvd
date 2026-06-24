import React from "react";
import { Instance, render } from "ink";
import { Picker } from "./Picker.js";

let INK_INSTANCE: Instance | null = null;

function unmount() {
  INK_INSTANCE?.unmount();
}

export function mountPicker(dirpath?: string) {
  INK_INSTANCE = render(<Picker dir={dirpath} unmount={unmount} />, {
    alternateScreen: true,
    exitOnCtrlC: false,
  });
}
