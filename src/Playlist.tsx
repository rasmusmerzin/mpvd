import * as path from "node:path";
import React, { useEffect, useMemo, useState } from "react";
import { Box, Key, Text, useInput } from "ink";
import {
  getPause,
  getPlaylist,
  playAtIndex,
  PlaylistItem,
  togglePause,
} from "./index.js";
import { useTerminalSize } from "./useTerminalSize.js";
import { mountPicker, mountPlaylist } from "./router.js";

export function Playlist({ unmount }: { unmount?: () => any }) {
  let [paused, setPaused] = useState(false);
  let [playlist, setPlaylist] = useState<PlaylistItem[]>([]);
  let [offset, setOffset] = useState(0);
  let [cursor, setCursor] = useState(0);
  const current = useMemo(
    () => playlist.findIndex(({ current }) => current) + 1,
    [playlist],
  );
  const [absolute, setAbsolute] = useState(false);
  const { rows } = useTerminalSize();
  const listHeight = rows - 1;
  const maxOffset = playlist.length - listHeight;
  function changeOffset(update = offset) {
    setOffset((offset = Math.max(0, Math.min(maxOffset, update))));
    changeCursor();
  }
  function changeCursor(pos = cursor) {
    const minCursor = offset;
    const maxCursor = Math.min(offset + listHeight - 1, playlist.length - 1);
    setCursor((cursor = Math.max(minCursor, Math.min(maxCursor, pos))));
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
      changeCursor(playlist.length - 1);
    } else if (input === "f") {
      setAbsolute(!absolute);
    } else if (input === "p") {
      mountPicker({ unmount: mountPlaylist });
    } else if (input === " ") {
      await togglePause();
      await updateState();
    } else if (key.return) {
      const position = cursor + 1;
      if (position === current) return togglePause();
      await playAtIndex(position);
      await updatePlaylist();
    }
  }
  async function updatePlaylist() {
    const list = await getPlaylist();
    if (Array.isArray(list)) setPlaylist((playlist = list));
  }
  async function updateState() {
    const value = await getPause();
    if (typeof value === "boolean") setPaused((paused = value));
  }
  async function update() {
    await updatePlaylist();
    await updateState();
  }
  useEffect(() => {
    const interval = setInterval(update, 500);
    update().then(() => {
      const position = playlist.findIndex(({ current }) => current);
      if (position >= 0) changeCursor(position);
    });
    setTimeout(update, 10);
    return () => {
      clearInterval(interval);
    };
  }, []);
  useInput(onInput);
  return (
    <>
      <Box height={listHeight} flexDirection="column">
        {playlist
          .slice(offset, offset + listHeight)
          .map(({ filename, current, id }, i) => (
            <PlaylistLine
              key={id}
              file={filename}
              absolute={absolute}
              current={current}
              index={i + 1}
              hover={cursor === i + offset}
              paused={paused}
            />
          ))}
      </Box>
    </>
  );
}

function PlaylistLine({
  hover = false,
  paused = false,
  absolute = false,
  current = false,
  file,
  index,
}: {
  hover?: boolean;
  paused?: boolean;
  absolute?: boolean;
  current?: boolean;
  file: string;
  index: number;
}) {
  const { columns } = useTerminalSize();
  let name = file;
  if (!absolute) name = path.basename(file);
  const indexStr = String(index).padStart(4, " ") + " ";
  const cursor = current ? (paused ? "- " : "* ") : "  ";
  const nameLength = columns - indexStr.length - cursor.length;
  const fixedName = name.slice(0, nameLength).padEnd(nameLength, " ");
  return (
    <Box>
      <Text
        inverse={hover}
        dimColor={!hover}
        color={current && hover ? "green" : undefined}
      >
        {indexStr}
      </Text>
      <Text inverse={hover} color={current && hover ? "green" : undefined}>
        {cursor}
      </Text>
      <Text inverse={hover} color={current ? "green" : undefined}>
        {fixedName}
      </Text>
    </Box>
  );
}
