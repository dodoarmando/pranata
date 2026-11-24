// Public entry chunk budget (D-06): what a visitor's first page load must run.
// The Playwright "public js budget" test checks the sum of all scripts too.
module.exports = [
	{
		name: "public entry chunk",
		path: "dist/client/assets/js/index.*.js",
		gzip: true,
		limit: "120 KB",
	},
];
