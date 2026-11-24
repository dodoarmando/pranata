// Swaps a `.yt-facade` link for the privacy-enhanced player on click, so nothing
// is requested from YouTube before the visitor asks for the video (D-20).
const VIDEO_ID = /^[\w-]{6,20}$/;

function onClick(event: MouseEvent) {
	if (event.defaultPrevented || event.button !== 0) {
		return;
	}
	const target = event.target;
	if (!(target instanceof Element)) {
		return;
	}
	const facade = target.closest<HTMLAnchorElement>("a.yt-facade");
	const id = facade?.dataset.videoId;
	if (!facade || !id || !VIDEO_ID.test(id)) {
		return;
	}
	event.preventDefault();
	const frame = document.createElement("iframe");
	frame.src = `https://www.youtube-nocookie.com/embed/${id}?autoplay=1`;
	frame.title = "Video YouTube";
	frame.allow = "autoplay; encrypted-media; picture-in-picture; fullscreen";
	frame.allowFullscreen = true;
	frame.referrerPolicy = "strict-origin-when-cross-origin";
	frame.className = "yt-frame";
	facade.replaceWith(frame);
}

let installed = false;

export function installYoutubeFacade() {
	if (installed || typeof document === "undefined") {
		return;
	}
	installed = true;
	document.addEventListener("click", onClick);
}
