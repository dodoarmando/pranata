import type { Village } from '../api/generated';
import { villageInitial, villageTitle } from '../village';

/**
 * The Village's logo, named in its alt text, or its initial until it has one. `className`
 * sizes both; `placeholderClassName` styles only the initial.
 */
export function VillageLogo({
  village,
  className,
  placeholderClassName,
}: {
  village: Village | null;
  className: string;
  placeholderClassName: string;
}) {
  return village?.logo ? (
    <img
      src={`/media/${village.logo}/480`}
      alt={`Logo ${villageTitle(village)}`}
      width={48}
      height={48}
      className={`${className} flex-none object-contain`}
    />
  ) : (
    <span
      className={`${className} ${placeholderClassName} grid flex-none place-items-center bg-primary text-primary-foreground`}
      aria-hidden="true"
    >
      {villageInitial(village)}
    </span>
  );
}
