// Public-page load test (D-07): 300 concurrent visitors on the VPS budget.
// Run with `just load`, which starts the stack and a pinned k6 image.
// R1 extends the scenario with news, gallery and search.

import { check, sleep } from "k6";
import http from "k6/http";

const BASE_URL = __ENV.BASE_URL || "https://localhost:8443";

export const options = {
	insecureSkipTLSVerify: true, // Caddy's internal CA
	scenarios: {
		visitors: {
			executor: "ramping-vus",
			startVUs: 0,
			stages: [
				{ duration: "30s", target: 300 },
				{ duration: "2m", target: 300 },
				{ duration: "15s", target: 0 },
			],
			gracefulRampDown: "10s",
		},
	},
	thresholds: {
		"http_req_duration{expected_response:true}": ["p(95)<1000"],
		http_req_failed: ["rate<0.01"],
	},
};

// One hero variant for every visitor: the browser would pick one from srcset.
export function setup() {
	const village = http.get(`${BASE_URL}/api/public/village`).json();
	const variants = village.hero?.variants || [];
	if (variants.length === 0) {
		throw new Error("the village has no hero photo; run load/seed.sh first");
	}
	const sorted = variants.slice().sort((a, b) => a.width - b.width);
	return { hero: sorted[Math.floor(sorted.length / 2)].url };
}

export default function (data) {
	const home = http.get(`${BASE_URL}/`);
	check(home, {
		"home is 200": (r) => r.status === 200,
		"home has the village heading": (r) => r.body.includes("<h1"),
	});
	const hero = http.get(`${BASE_URL}${data.hero}`);
	check(hero, { "hero is 200": (r) => r.status === 200 });
	sleep(1 + Math.random() * 2);
}
