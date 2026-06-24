import * as path from "node:path";
import React, { useEffect, useMemo, useState } from "react";
import { Box, Key, Text, useInput } from "ink";
import { Input } from "./Input.js";
import { find, isMpvPlayableAudio } from "./find.js";
import { pushToPlaylist } from "./index.js";
import { shuffle } from "./shuffle.js";
import { startDaemon } from "./index.js";
import { useTerminalSize } from "./useTerminalSize.js";

export function Picker({
  dir = "~/Music",
  unmount,
}: {
  dir?: string;
  unmount?: () => any;
}) {
  const { rows } = useTerminalSize();
  let [files, setFiles] = useState<string[]>([]);
  let [offset, setOffset] = useState(0);
  let [cursor, setCursor] = useState(0);
  let [shuffled, setShuffled] = useState(false);
  const [absolute, setAbsolute] = useState(false);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [mode, setMode] = useState<"main" | "search">("main");
  const [search, setSearch] = useState("");
  const searchRegex = useMemo(
    () => (search ? new RegExp(search) : null),
    [search],
  );
  const filteredFiles = useMemo(() => {
    setTimeout(changeOffset);
    return searchRegex
      ? files.filter((f) => searchRegex.test(path.basename(f)))
      : files;
  }, [files, searchRegex]);
  const listHeight = rows - 1;
  const maxOffset = filteredFiles.length - listHeight;
  function changeOffset(update = offset) {
    setOffset((offset = Math.max(0, Math.min(maxOffset, update))));
    changeCursor();
  }
  function changeCursor(pos = cursor) {
    const minCursor = offset;
    const maxCursor = Math.min(
      offset + listHeight - 1,
      filteredFiles.length - 1,
    );
    setCursor((cursor = Math.max(minCursor, Math.min(maxCursor, pos))));
  }
  async function updateFiles(dirname: string) {
    const list = await find(dirname, isMpvPlayableAudio);
    setFiles((files = list));
  }
  async function toggleShuffle(value = !shuffled) {
    setShuffled((shuffled = value));
    await updateFiles(dir);
    const relative = cursor - offset;
    changeOffset(0);
    changeCursor(relative);
    if (shuffled) setFiles((files = shuffle(files)));
  }
  async function onInput(input: string, key: Key) {
    if (key.escape || input === "q" || (input === "c" && key.ctrl)) {
      unmount?.();
    } else if (input === "e" && key.ctrl) {
      changeOffset(offset + 1);
    } else if (input === "y" && key.ctrl) {
      changeOffset(offset - 1);
    } else if (input === "d" && key.ctrl) {
      const delta = (rows / 2) | 0;
      const savedCursor = cursor;
      changeOffset(offset + delta);
      changeCursor(savedCursor + delta);
    } else if (input === "u" && key.ctrl) {
      const delta = -((rows / 2) | 0);
      const savedCursor = cursor;
      changeOffset(offset + delta);
      changeCursor(savedCursor + delta);
    } else if (input === "H") {
      changeCursor(offset);
    } else if (input === "L") {
      changeCursor(offset + listHeight);
    } else if (key.downArrow || input === "j" || (input === "n" && key.ctrl)) {
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
      changeCursor(filteredFiles.length - 1);
    } else if (input === "f") {
      setAbsolute(!absolute);
    } else if (input === "r") {
      toggleShuffle();
    } else if (input === " " || key.tab) {
      if (selected.has(filteredFiles[cursor]))
        selected.delete(filteredFiles[cursor]);
      else selected.add(filteredFiles[cursor]);
      setSelected(new Set(selected));
    } else if (key.return) {
      unmount?.();
      const started = await startDaemon();
      if (started) await new Promise((r) => setTimeout(r, 200));
      await Promise.all(
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
    setSearch("");
    setMode("main");
  }
  function onSearchSubmit(text: string) {
    setSearch(text);
    setMode("main");
  }
  function onSearchChange(text: string) {
    setSearch(text);
  }
  useEffect(() => {
    updateFiles(dir);
  }, [dir]);
  useInput(onInput, { isActive: mode === "main" });
  return (
    <>
      <Box height={listHeight} flexDirection="column">
        {filteredFiles.slice(offset, offset + listHeight).map((file, i) => (
          <PickerLine
            key={file}
            file={file}
            absolute={absolute}
            selected={selected.has(file)}
            hover={cursor === i + offset}
          />
        ))}
      </Box>
      {mode === "search" ? (
        <Input
          prefix="/"
          onCancel={onSearchCancel}
          onSubmit={onSearchSubmit}
          onChange={onSearchChange}
        />
      ) : (
        search && <Text>/{search}</Text>
      )}
    </>
  );
}

function PickerLine({
  selected = false,
  hover = false,
  absolute = false,
  file,
}: {
  selected?: boolean;
  hover?: boolean;
  absolute?: boolean;
  file: string;
}) {
  const { columns } = useTerminalSize();
  const prefix = selected ? "* " : "  ";
  let name = file;
  if (!absolute) name = path.basename(file);
  const full = prefix + name;
  return (
    <Text inverse={hover}>{full.slice(0, columns).padEnd(columns, " ")}</Text>
  );
}
