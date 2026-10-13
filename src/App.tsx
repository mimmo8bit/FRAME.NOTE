import { useState } from "react";
import Home from "./windows/Home";
import Diagnostica from "./windows/Diagnostica";
import ProvaZoom from "./windows/ProvaZoom";

type Screen = "home" | "diagnostica" | "prova-zoom";

export default function App() {
  const [screen, setScreen] = useState<Screen>("home");
  const home = () => setScreen("home");
  switch (screen) {
    case "diagnostica":
      return <Diagnostica onBack={home} />;
    case "prova-zoom":
      return <ProvaZoom onBack={home} />;
    default:
      return (
        <Home
          onOpenDiagnostics={() => setScreen("diagnostica")}
          onOpenZoomTest={() => setScreen("prova-zoom")}
        />
      );
  }
}
