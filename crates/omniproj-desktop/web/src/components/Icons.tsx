import type { SVGProps } from "react";

type IconProps = SVGProps<SVGSVGElement>;

function Icon({ children, ...props }: IconProps) {
  return (
    <svg
      aria-hidden="true"
      fill="none"
      viewBox="0 0 20 20"
      stroke="currentColor"
      strokeWidth="1.5"
      strokeLinecap="round"
      strokeLinejoin="round"
      {...props}
    >
      {children}
    </svg>
  );
}

export function SidebarIcon(props: IconProps) {
  return <Icon {...props}><rect x="2.75" y="3" width="14.5" height="14" rx="2" /><path d="M7.25 3v14" /></Icon>;
}

export function ChevronLeftIcon(props: IconProps) {
  return <Icon {...props}><path d="m12 5-5 5 5 5" /></Icon>;
}

export function ChevronRightIcon(props: IconProps) {
  return <Icon {...props}><path d="m8 5 5 5-5 5" /></Icon>;
}

export function ChevronDownIcon(props: IconProps) {
  return <Icon {...props}><path d="m5 8 5 5 5-5" /></Icon>;
}

export function FolderIcon(props: IconProps) {
  return <Icon {...props}><path d="M2.75 6.5h5l1.5-2h8v11.25H2.75z" /></Icon>;
}

export function PlusIcon(props: IconProps) {
  return <Icon {...props}><path d="M10 4v12M4 10h12" /></Icon>;
}

export function RefreshIcon(props: IconProps) {
  return <Icon {...props}><path d="M15.5 6.25V3.5m0 0h-2.75m2.75 0-2.2 2.2a5.75 5.75 0 1 0 1.5 5.55" /></Icon>;
}

export function GearIcon(props: IconProps) {
  return <Icon {...props}><circle cx="10" cy="10" r="2.5" /><path d="M10 2.75v2M10 15.25v2M2.75 10h2M15.25 10h2M4.9 4.9l1.4 1.4M13.7 13.7l1.4 1.4M15.1 4.9l-1.4 1.4M6.3 13.7l-1.4 1.4" /></Icon>;
}
