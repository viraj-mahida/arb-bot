import { useEffect, useState } from "react";
import { useEventSource } from "./events";
import { Hud } from "./hud/Hud";
import { Stage } from "./scene/Stage";

export function App() {
  useEventSource();
  const scale = useStageScale();
  return (
    <div className="viewport">
      <div style={{ width: 1920 * scale, height: 1080 * scale }}>
        <div className="stage" style={{ transform: `scale(${scale})` }}>
          <Stage />
          <Hud />
        </div>
      </div>
    </div>
  );
}

function useStageScale() {
  const [scale, setScale] = useState(1);
  useEffect(() => {
    const fit = () => setScale(Math.min(window.innerWidth / 1920, window.innerHeight / 1080));
    fit();
    window.addEventListener("resize", fit);
    return () => window.removeEventListener("resize", fit);
  }, []);
  return scale;
}
