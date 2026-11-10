import { renderFrame } from "./frame.tsx";
import { paintAccent, paintTheme, savedAccent, savedTheme } from "./site.ts";
import { Site } from "./Site.tsx";
import { createRoot } from "react-dom/client";
import "./site.css";

const story = new URLSearchParams(window.location.search).get("story");
const root = document.getElementById("root");

if (story !== null) {
	void renderFrame(story);
} else if (root !== null) {
	paintTheme(savedTheme());
	paintAccent(savedAccent());
	createRoot(root).render(<Site />);
}
