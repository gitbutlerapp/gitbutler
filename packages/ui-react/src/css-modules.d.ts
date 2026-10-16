// TODO: use https://github.com/mizdra/css-modules-kit
declare module "*.module.css" {
	const classes: Record<string, string>;
	export default classes;
}
