import React, { useEffect, useState } from "react";
import { Key, Text, useInput } from "ink";
import * as path from "node:path";
import { useTerminalSize } from "./useTerminalSize.js";
import { find, isMpvPlayableAudio } from "./find.js";
import { shuffle } from "./shuffle.js";
import { unmount } from "./mpvd.js";
import { pushToPlaylist } from "./index.js";
import { Input } from "./Input.js";

export function PickerLine({
  selected,
  hover,
  file,
}: {
  selected: boolean;
  hover: boolean;
  file: string;
}) {
  const { columns } = useTerminalSize();
  const prefix = selected ? "* " : "  ";
  const full = prefix + path.basename(file).slice(0, columns - 2);
  return <Text inverse={hover}>{full.padEnd(columns, " ")}</Text>;
}

export function Picker() {
  const { rows } = useTerminalSize();
  let [files, setFiles] = useState<string[]>([]);
  let [offset, setOffset] = useState(0);
  let [cursor, setCursor] = useState(0);
  let [shuffled, setShuffled] = useState(false);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [mode, setMode] = useState<"main" | "search">("main");
  const [search, setSearch] = useState("");
  const listHeight = rows - 1;
  const maxOffset = files.length - listHeight;
  function changeOffset(update: number) {
    setOffset((offset = Math.max(0, Math.min(maxOffset, update))));
    changeCursor(cursor);
  }
  function changeCursor(pos: number) {
    const minCursor = offset;
    const maxCursor = Math.min(offset + listHeight - 1, files.length);
    setCursor((cursor = Math.max(minCursor, Math.min(maxCursor, pos))));
  }
  async function updateFiles() {
    const list = await find("~/Music", isMpvPlayableAudio);
    setFiles((files = list));
    const relative = cursor - offset;
    changeOffset(0);
    changeCursor(relative);
  }
  async function toggleShuffle(value = !shuffled) {
    setShuffled((shuffled = value));
    await updateFiles();
    if (shuffled) setFiles((files = shuffle(files)));
  }
  function onInput(input: string, key: Key) {
    if (input === "e" && key.ctrl) changeOffset(offset + 1);
    else if (input === "y" && key.ctrl) changeOffset(offset - 1);
    else if (input === "d" && key.ctrl) {
      const delta = (rows / 2) | 0;
      const savedCursor = cursor;
      changeOffset(offset + delta);
      changeCursor(savedCursor + delta);
    } else if (input === "u" && key.ctrl) {
      const delta = -((rows / 2) | 0);
      const savedCursor = cursor;
      changeOffset(offset + delta);
      changeCursor(savedCursor + delta);
    } else if (input === "r") {
      toggleShuffle();
    } else if (key.escape || input === "q" || (input === "c" && key.ctrl))
      unmount();
    else if (input === "H") changeCursor(offset);
    else if (input === "L") changeCursor(offset + listHeight);
    else if (key.downArrow || input === "j" || (input === "n" && key.ctrl)) {
      if (cursor + 1 >= offset + listHeight) changeOffset(offset + 1);
      changeCursor(cursor + 1);
    } else if (key.upArrow || input === "k" || (input === "p" && key.ctrl)) {
      if (cursor - 1 < offset) changeOffset(offset - 1);
      changeCursor(cursor - 1);
    } else if (input === "g") {
      changeOffset(0);
      changeCursor(0);
    } else if (input === "G") {
      changeOffset(maxOffset);
      changeCursor(files.length - 1);
    } else if (input === " " || key.tab) {
      if (selected.has(files[cursor])) selected.delete(files[cursor]);
      else selected.add(files[cursor]);
      setSelected(new Set(selected));
    } else if (key.return) {
      unmount();
      Promise.all(
        Array.from(selected, async (file) => {
          await pushToPlaylist(file);
          console.log(file);
        }),
      );
    } else if (input === "/") {
      setMode("search");
    }
  }
  function onSearchCancel() {
    setMode("main");
  }
  function onSearchSubmit(text: string) {
    setSearch(text);
    setMode("main");
  }
  useEffect(() => {
    updateFiles();
  }, []);
  useInput(onInput, { isActive: mode === "main" });
  return (
    <>
      {files.slice(offset, offset + listHeight).map((file, i) => (
        <PickerLine
          key={file}
          file={file}
          selected={selected.has(file)}
          hover={cursor === i + offset}
        />
      ))}
      {mode === "search" ? (
        <Input prefix="/" onCancel={onSearchCancel} onSubmit={onSearchSubmit} />
      ) : (
        search && <Text>/{search}</Text>
      )}
    </>
  );
}
