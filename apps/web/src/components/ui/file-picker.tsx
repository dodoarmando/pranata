import { type ComponentProps, useRef } from "react";
import { Button } from "@/components/ui/button";

type FilePickerProps = Omit<ComponentProps<typeof Button>, "onClick"> & {
	accept?: string;
	multiple?: boolean;
	onFiles: (files: File[]) => void;
};

// A Button that opens the native file chooser. The input stays visually hidden
// and is reset after each pick so the same photo can be chosen again.
function FilePicker({
	accept,
	multiple,
	onFiles,
	children,
	disabled,
	...props
}: FilePickerProps) {
	const input = useRef<HTMLInputElement>(null);
	return (
		<>
			<input
				ref={input}
				type="file"
				accept={accept}
				multiple={multiple}
				className="sr-only"
				tabIndex={-1}
				aria-hidden="true"
				onChange={(event) => {
					const files = Array.from(event.currentTarget.files ?? []);
					event.currentTarget.value = "";
					if (files.length > 0) onFiles(files);
				}}
			/>
			<Button
				type="button"
				disabled={disabled}
				onClick={() => input.current?.click()}
				{...props}
			>
				{children}
			</Button>
		</>
	);
}

export { FilePicker };
