import { useState } from "react";
import Home from "./windows/Home";
import Diagnostica from "./windows/Diagnostica";
import ProvaZoom from "./windows/ProvaZoom";
import Privacy from "./windows/Privacy";

type Screen = "home" | "diagnostica" | "prova-zoom" | "privacy";

export default function App() {
  const [screen, setScreen] = useState<Screen>("home");
  const home = () => setScreen("home");
  switch (screen) {
    case "diagnostica":
      return <Diagnostica onBack={home} />;
    case "prova-zoom":
      return <ProvaZoom onBack={home} />;
    case "privacy":
      return <Privacy onBack={home} />;
    default:
      return (
        <Home
          onOpenDiagnostics={() => setScreen("diagnostica")}
          onOpenZoomTest={() => setScreen("prova-zoom")}
          onOpenPrivacy={() => setScreen("privacy")}
        />
      );
  }
}
