import { ReviewApp } from "./ReviewApp.tsx";
import { createRoot } from "react-dom/client";
import "./styles.css";
import "./review.css";

const root = document.getElementById("root");

if (root === null) {
	throw new Error("GitButler MCP App root element is missing");
}

createRoot(root).render(<ReviewApp />);
