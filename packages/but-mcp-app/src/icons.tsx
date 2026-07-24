import type { ForgeInfo } from "@gitbutler/but-sdk";
import type { ComponentProps, ReactNode } from "react";

type IconProps = ComponentProps<"svg"> & {
	children: ReactNode;
};

function Icon({ children, ...props }: IconProps) {
	return (
		<svg {...props} fill="none" viewBox="0 0 16 16" width="100%" height="100%" aria-hidden="true">
			{children}
		</svg>
	);
}

export function ForgeIcon({ name, ...props }: { name: ForgeInfo["name"] } & ComponentProps<"svg">) {
	switch (name) {
		case "github":
			return (
				<Icon {...props}>
					<path
						stroke="currentColor"
						strokeWidth="1.5"
						d="M5.57 15v-1.293a3 3 0 0 1 1.32-2.486l.117-.08c.078-.052.048-.173-.045-.185-1.91-.246-4.202-1.079-4.202-4.285 0-.956.35-1.738.905-2.347-.088-.217-.394-1.115.087-2.317 1.489-.097 2.424.898 2.424.898a8.6 8.6 0 0 1 4.408 0s.782-.954 2.423-.898c.482 1.202.175 2.1.088 2.317.57.609.905 1.391.905 2.347 0 3.21-2.28 4.027-4.202 4.284-.092.012-.121.133-.044.185l.119.08a3 3 0 0 1 1.317 2.484V15"
						vectorEffect="non-scaling-stroke"
					/>
					<path
						stroke="currentColor"
						strokeLinecap="round"
						strokeWidth="1.5"
						d="M5 14c-2.409 0-1.83-2.248-3.372-2.248"
						vectorEffect="non-scaling-stroke"
					/>
				</Icon>
			);
		case "gitlab":
			return (
				<Icon {...props}>
					<path
						stroke="currentColor"
						strokeWidth="1.5"
						d="m2.652 9.695 5.223 4.204a.2.2 0 0 0 .25 0l5.21-4.195a2 2 0 0 0 .595-2.32l-2.025-4.912a.2.2 0 0 0-.372.007l-1.366 3.684h-4.22L4.4 2.45a.2.2 0 0 0-.37.003L2.05 7.394a2 2 0 0 0 .602 2.3Z"
						vectorEffect="non-scaling-stroke"
					/>
				</Icon>
			);
		case "bitbucket":
			return (
				<Icon {...props}>
					<path
						stroke="currentColor"
						strokeWidth="1.5"
						d="m13.308 7.406-.894 5.56c-.059.336-.292.534-.622.534H4.208c-.33 0-.564-.198-.622-.534L2.011 3.054c-.058-.336.117-.554.428-.554H13.56c.311 0 .486.218.428.554l-.428 2.611c-.058.376-.272.535-.622.535H6.25c-.097 0-.156.059-.136.178l.525 3.284c.02.079.078.138.155.138h2.412c.077 0 .136-.059.155-.138l.37-2.374c.038-.297.233-.416.505-.416h2.625c.389 0 .506.198.447.534Z"
						vectorEffect="non-scaling-stroke"
					/>
				</Icon>
			);
		case "azure":
			return (
				<Icon {...props}>
					<circle
						cx="8"
						cy="8"
						r="6.25"
						stroke="currentColor"
						strokeWidth="1.5"
						vectorEffect="non-scaling-stroke"
					/>
					<path
						stroke="currentColor"
						strokeWidth="1.5"
						d="M6 7.2v-1a2 2 0 1 1 4 0c0 .609-.182 1.182-.621 1.621L8 9.2"
						vectorEffect="non-scaling-stroke"
					/>
					<circle cx="8" cy="11.2" r="1" fill="currentColor" />
				</Icon>
			);
	}
}

export function OpenInBrowserIcon(props: ComponentProps<"svg">) {
	return (
		<Icon {...props}>
			<path
				stroke="currentColor"
				strokeWidth="1.5"
				d="m11.536 12.243.609-7.308a1 1 0 0 0-1.08-1.08l-7.308.61M4 12l8-8"
				vectorEffect="non-scaling-stroke"
			/>
		</Icon>
	);
}

export function BranchIcon(props: ComponentProps<"svg">) {
	return (
		<Icon {...props}>
			<path
				stroke="currentColor"
				strokeWidth="1.5"
				d="M2.75 10.5h5.03a2 2 0 0 0 1.909-1.404L10.25 7.3m-7.5 3.2V2m0 8.5V14"
				vectorEffect="non-scaling-stroke"
			/>
			<circle
				cx="10.75"
				cy="4.5"
				r="2.5"
				stroke="currentColor"
				strokeWidth="1.5"
				vectorEffect="non-scaling-stroke"
			/>
		</Icon>
	);
}

export function CiIcon({
	kind,
	...props
}: { kind: "cross" | "question" | "spinner" | "tick" | "warning" } & ComponentProps<"svg">) {
	switch (kind) {
		case "tick":
			return (
				<Icon {...props}>
					<path
						d="M14 4 6.8 11 2 6.333"
						stroke="currentColor"
						strokeWidth="1.5"
						vectorEffect="non-scaling-stroke"
					/>
				</Icon>
			);
		case "cross":
			return (
				<Icon {...props}>
					<path
						d="M13.5 2.5 8 8m0 0-5.5 5.5M8 8l5.5 5.5M8 8 2.5 2.5"
						stroke="currentColor"
						strokeWidth="1.5"
						vectorEffect="non-scaling-stroke"
					/>
				</Icon>
			);
		case "spinner":
			return (
				<Icon {...props}>
					<path
						d="M13 8a5 5 0 1 1-5-5"
						stroke="currentColor"
						strokeWidth="1.5"
						vectorEffect="non-scaling-stroke"
					/>
				</Icon>
			);
		case "warning":
			return (
				<Icon {...props}>
					<path
						d="M6.896 2.677c.47-.885 1.737-.885 2.208 0l5.172 9.736a1.25 1.25 0 0 1-1.103 1.837H2.827a1.25 1.25 0 0 1-1.103-1.837l5.172-9.736Z"
						stroke="currentColor"
						strokeWidth="1.5"
						vectorEffect="non-scaling-stroke"
					/>
					<path
						d="M8 9.5V7m0 5v-1.25"
						stroke="currentColor"
						strokeWidth="1.5"
						vectorEffect="non-scaling-stroke"
					/>
				</Icon>
			);
		case "question":
			return (
				<Icon {...props}>
					<circle
						cx="8"
						cy="8"
						r="6.25"
						stroke="currentColor"
						strokeWidth="1.5"
						vectorEffect="non-scaling-stroke"
					/>
					<path
						stroke="currentColor"
						strokeWidth="1.5"
						d="M6 7.2v-1a2 2 0 1 1 4 0c0 .609-.182 1.182-.621 1.621L8 9.2"
						vectorEffect="non-scaling-stroke"
					/>
					<circle cx="8" cy="11.2" r="1" fill="currentColor" />
				</Icon>
			);
	}
}
