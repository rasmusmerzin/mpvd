import * as path from "node:path";
import React, { useEffect, useMemo, useState } from "react";
import { Box, Key, Text, useInput } from "ink";
import {
  MpvObserver,
  PlaylistItem,
  formatTimeString,
  getPlaylist,
  moveInPlaylist,
  playAtIndex,
  removeFromPlaylist,
  seek,
  togglePause,
} from "./index.js";
import { useTerminalSize } from "./useTerminalSize.js";
import { mountPicker, mountPlaylist } from "./router.js";

export function Playlist({ unmount }: { unmount?: () => any }) {
  let [time, setTime] = useState(0);
  let [duration, setDuration] = useState(0);
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
  function changeOffset(value = offset) {
    setOffset((offset = Math.max(0, Math.min(maxOffset, value))));
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
    } else if (
      (key.downArrow && !key.shift) ||
      input === "j" ||
      (input === "n" && key.ctrl)
    ) {
      if (cursor + 1 >= offset + listHeight) changeOffset(offset + 1);
      changeCursor(cursor + 1);
    } else if (
      (key.upArrow && !key.shift) ||
      input === "k" ||
      (input === "p" && key.ctrl)
    ) {
      if (cursor - 1 < offset) changeOffset(offset - 1);
      changeCursor(cursor - 1);
    } else if (input === "g") {
      changeOffset(0);
      changeCursor(0);
    } else if (input === "G") {
      changeOffset(maxOffset);
      changeCursor(playlist.length - 1);
    } else if (input === "f" && !key.ctrl) {
      setAbsolute(!absolute);
    } else if (input === "p") {
      mountPicker({ unmount: mountPlaylist });
    } else if (input === "J" || (key.downArrow && key.shift)) {
      const position = cursor + 1;
      if (position >= playlist.length) return;
      await moveInPlaylist(position, position + 1);
      await updatePlaylist();
      changeCursor(cursor + 1);
    } else if (input === "K" || (key.upArrow && key.shift)) {
      const position = cursor + 1;
      if (position < 2) return;
      await moveInPlaylist(position, position - 1);
      await updatePlaylist();
      changeCursor(cursor - 1);
    } else if (input === "D" || key.delete) {
      const position = cursor + 1;
      await removeFromPlaylist(position);
      setTimeout(changeCursor);
    } else if (input === " ") {
      await togglePause();
    } else if (key.leftArrow || (input === "b" && key.ctrl)) {
      await seek(-5);
    } else if (key.rightArrow || (input === "f" && key.ctrl)) {
      await seek(5);
    } else if (key.return) {
      const position = cursor + 1;
      if (position === current) return togglePause();
      await playAtIndex(position);
    }
  }
  async function updatePlaylist(list?: PlaylistItem[]) {
    const wasEmpty = !playlist.length;
    if (!list) list = await getPlaylist();
    setPlaylist((playlist = list));
    if (!wasEmpty) return;
    const position = playlist.findIndex(({ current }) => current);
    if (position >= 0) changeCursor(position);
  }
  useEffect(() => {
    const observer = new MpvObserver();
    function onConnect() {
      observer.observe("playlist", updatePlaylist);
      observer.observe("pause", setPaused);
      observer.observe("time-pos", setTime);
      observer.observe("duration", setDuration);
    }
    observer.addEventListener("connect", onConnect, { once: true });
    return () => observer.close();
  }, []);
  useInput(onInput);
  return (
    <>
      <Box height={listHeight} flexDirection="column">
        {!playlist.length && (
          <Text italic dimColor>
            Playlist is empty. Press p to pick files.
          </Text>
        )}
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
      <PlaylistStatusLine
        file={playlist[current - 1]?.filename}
        index={current}
        absolute={absolute}
        paused={paused}
        time={time}
        duration={duration}
      />
    </>
  );
}

function PlaylistLine({
  index = 0,
  file = "",
  hover = false,
  paused = false,
  absolute = false,
  current = false,
}: {
  file: string;
  index: number;
  hover?: boolean;
  paused?: boolean;
  absolute?: boolean;
  current?: boolean;
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

function PlaylistStatusLine({
  index,
  file = "",
  absolute = false,
  paused = false,
  time = 0,
  duration = 0,
}: {
  index: number;
  file?: string;
  absolute?: boolean;
  paused?: boolean;
  time?: number;
  duration?: number;
}) {
  const { columns } = useTerminalSize();
  const indexStr = String(index).padStart(4, " ") + " ";
  const timeStr = " " + formatTimeString(time, duration);
  const cursor = paused ? "- " : "* ";
  let name = file;
  if (!absolute) name = path.basename(file);
  const nameLength = columns - indexStr.length - cursor.length - timeStr.length;
  const fixedName = name.slice(0, nameLength).padEnd(nameLength, " ");
  const status = indexStr + cursor + fixedName + timeStr;
  return (
    <Box>
      <Text>{status}</Text>
    </Box>
  );
}
