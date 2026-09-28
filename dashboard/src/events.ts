import { useEffect } from "react";
import { armAudio } from "./audio";
import { useCity } from "./store";
import type { BotEvent } from "./types";

const SOCKET_URL = "ws://127.0.0.1:8787/ws";

function sleep(ms: number) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

async function playRecording(text: string, speed: number) {
  const events: BotEvent[] = text
    .split("\n")
    .map((line) => line.trim())
    .filter(Boolean)
    .map((line) => JSON.parse(line) as BotEvent);
  let previous = events[0]?.t ?? 0;
  for (const event of events) {
    const delta = Math.min(900, Math.max(0, (event.t ?? previous) - previous));
    previous = event.t ?? previous;
    if (delta > 0) await sleep(delta / speed);
    useCity.getState().apply(event);
  }
}

export function useEventSource() {
  const setLink = useCity((state) => state.setLink);

  useEffect(() => {
    const onPointer = () => armAudio();
    window.addEventListener("pointerdown", onPointer);
    const params = new URLSearchParams(window.location.search);
    let stopped = false;

    if (params.has("preview")) {
      setLink("preview");
      void fetch("/preview.jsonl")
        .then((response) => response.text())
        .then((text) => {
          if (!stopped) return playRecording(text, 1);
        });
      return () => {
        stopped = true;
        window.removeEventListener("pointerdown", onPointer);
      };
    }

    if (params.has("replay")) {
      setLink("replay");
      const which = params.get("replay") || "1";
      const url = which === "1" ? "http://127.0.0.1:8787/events.jsonl" : which;
      void fetch(url)
        .then((response) => {
          if (!response.ok) throw new Error(`replay ${response.status}`);
          return response.text();
        })
        .then((text) => {
          if (!stopped) return playRecording(text, 1);
        })
        .catch(() => setLink("offline"));
      return () => {
        stopped = true;
        window.removeEventListener("pointerdown", onPointer);
      };
    }

    let socket: WebSocket | null = null;
    let timer = 0;
    const connect = () => {
      if (stopped) return;
      setLink("connecting");
      socket = new WebSocket(SOCKET_URL);
      socket.onopen = () => setLink("live");
      socket.onmessage = (message) => {
        try {
          useCity.getState().apply(JSON.parse(String(message.data)) as BotEvent);
        } catch {
          /* ignore a malformed line */
        }
      };
      socket.onclose = () => {
        setLink("offline");
        timer = window.setTimeout(connect, 1500);
      };
    };
    connect();
    return () => {
      stopped = true;
      window.clearTimeout(timer);
      socket?.close();
      window.removeEventListener("pointerdown", onPointer);
    };
  }, [setLink]);
}

export function downloadTake() {
  const lines = useCity
    .getState()
    .take.map((event) => JSON.stringify(event))
    .join("\n");
  const blob = new Blob([lines + "\n"], { type: "application/x-ndjson" });
  const url = URL.createObjectURL(blob);
  const link = document.createElement("a");
  link.href = url;
  link.download = "rbot-take.jsonl";
  link.click();
  URL.revokeObjectURL(url);
}
