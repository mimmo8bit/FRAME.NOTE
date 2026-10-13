import { useState } from "react";
import Home from "./windows/Home";
import Diagnostica from "./windows/Diagnostica";

type Screen = "home" | "diagnostica";

export default function App() {
  const [screen, setScreen] = useState<Screen>("home");
  return screen === "home" ? (
    <Home onOpenDiagnostics={() => setScreen("diagnostica")} />
  ) : (
    <Diagnostica onBack={() => setScreen("home")} />
  );
}
