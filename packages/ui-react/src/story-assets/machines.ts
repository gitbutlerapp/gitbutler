import docker from "./machines/docker.webp";
import imac from "./machines/imac.webp";
import laptop from "./machines/laptop.webp";
import lxc from "./machines/lxc.webp";
import macMini from "./machines/mac-mini.webp";
import macStudio from "./machines/mac-studio.webp";
import macbook from "./machines/macbook.webp";
import miniPc from "./machines/mini-pc.webp";
import miniPcSilver from "./machines/mini-pc-silver.webp";
import nas from "./machines/nas.webp";
import rack from "./machines/rack.webp";
import sff from "./machines/sff.webp";
import singleBoard from "./machines/single-board.webp";
import tower from "./machines/tower.webp";
import vm from "./machines/vm.webp";

/**
 * The machine pictures, exported at 76px (the 38px avatar at 2×) from the Machines set on ⚛️ Core's
 * Icons page, in the order but.dev's picker offers them, so the stories show the picker as but.dev
 * fills it. The pictures are but.dev's to own; the library only lays them out.
 */
export const machinePictures = [
	{ name: "Laptop", src: laptop },
	{ name: "MacBook", src: macbook },
	{ name: "iMac", src: imac },
	{ name: "Mac mini", src: macMini },
	{ name: "Mac Studio", src: macStudio },
	{ name: "Mini PC", src: miniPc },
	{ name: "Mini PC, silver", src: miniPcSilver },
	{ name: "Small form factor", src: sff },
	{ name: "Tower", src: tower },
	{ name: "NAS", src: nas },
	{ name: "Single-board computer", src: singleBoard },
	{ name: "Rack server", src: rack },
	{ name: "Virtual machine", src: vm },
	{ name: "Docker container", src: docker },
	{ name: "LXC container", src: lxc },
] as const;
