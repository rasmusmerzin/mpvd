import React, { useState } from "react";
import { Box, Key, Text, useInput } from "ink";
import { useTerminalSize } from "./useTerminalSize.js";

export function Input({
  isActive = true,
  onSubmit,
  onChange,
  onCancel,
  prefix = "",
}: {
  isActive?: boolean;
  onSubmit?: (text: string) => any;
  onChange?: (text: string) => any;
  onCancel?: () => any;
  prefix?: string;
}) {
  const { columns } = useTerminalSize();
  let [text, setText] = useState("");
  let [position, setPosition] = useState(0);
  const maxLength = columns - prefix.length;
  function changePosition(value = position) {
    const maxPosition = Math.min(maxLength - 1, text.length);
    setPosition((position = Math.max(0, Math.min(maxPosition, value))));
  }
  function insertText(value: string) {
    const left = text.slice(0, position);
    const right = text.slice(position);
    setText((text = left + value + right));
    changePosition(position + value.length);
    onChange?.(text);
  }
  function deleteCharBackward() {
    const left = text.slice(0, position - 1);
    const right = text.slice(position);
    setText((text = left + right));
    changePosition(position - 1);
    onChange?.(text);
  }
  function deleteCharForward() {
    const left = text.slice(0, position);
    const right = text.slice(position + 1);
    setText((text = left + right));
    onChange?.(text);
  }
  function deleteBackward() {
    const right = text.slice(position);
    setText((text = right));
    changePosition(0);
    onChange?.(text);
  }
  function deleteForward() {
    const left = text.slice(0, position);
    setText((text = left));
    onChange?.(text);
  }
  function deleteWordBackward() {
    const left = text
      .slice(0, position)
      .trimEnd()
      .replace(/[^\s]+$/, "");
    const right = text.slice(position);
    setText((text = left + right));
    onChange?.(text);
  }
  function onInput(input: string, key: Key) {
    if (!key.ctrl && input && !key.return) {
      insertText(input);
    } else if (key.return) {
      onSubmit?.(text);
    } else if (key.escape || input === "c") {
      onCancel?.();
    } else if (key.leftArrow || input === "b") {
      changePosition(position - 1);
    } else if (key.rightArrow || input === "f") {
      changePosition(position + 1);
    } else if (input === "a") {
      changePosition(0);
    } else if (input === "e") {
      changePosition(maxLength);
    } else if (key.backspace || input === "h") {
      deleteCharBackward();
    } else if (key.delete || input === "d") {
      deleteCharForward();
    } else if (input === "u") {
      deleteBackward();
    } else if (input === "k") {
      deleteForward();
    } else if (input === "w") {
      deleteWordBackward();
    }
  }
  useInput(onInput, { isActive });
  const display = text.slice(0, maxLength);
  const left = display.slice(0, position);
  const cursor = display.slice(position, position + 1) || " ";
  const right = display.slice(position + 1);
  return (
    <Box>
      <Text>{prefix}</Text>
      <Text>{left}</Text>
      <Text inverse>{cursor}</Text>
      <Text>{right}</Text>
    </Box>
  );
}
