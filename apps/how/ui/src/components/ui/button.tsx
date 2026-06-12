import { cn } from "#ui/lib/utils.ts";
import { Slot } from "@radix-ui/react-slot";
import { cva, type VariantProps } from "class-variance-authority";
import type { ButtonHTMLAttributes } from "react";

const buttonVariants = cva(
	"inline-flex h-10 items-center justify-center gap-2 whitespace-nowrap rounded-md px-4 text-sm font-medium transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-stone-800 disabled:pointer-events-none disabled:opacity-50",
	{
		variants: {
			variant: {
				default: "bg-stone-950 text-stone-50 hover:bg-stone-800",
				secondary: "border border-stone-300 bg-white text-stone-950 hover:bg-stone-100",
				ghost: "text-stone-700 hover:bg-stone-100 hover:text-stone-950",
			},
			size: {
				default: "h-10 px-4",
				sm: "h-8 px-3 text-xs",
				icon: "h-9 w-9 px-0",
			},
		},
		defaultVariants: {
			variant: "default",
			size: "default",
		},
	},
);

type ButtonProps = ButtonHTMLAttributes<HTMLButtonElement> &
	VariantProps<typeof buttonVariants> & {
		asChild?: boolean;
	};

export function Button({
	className,
	variant,
	size,
	asChild = false,
	...props
}: ButtonProps) {
	const Comp = asChild ? Slot : "button";
	return <Comp className={cn(buttonVariants({ variant, size, className }))} {...props} />;
}
