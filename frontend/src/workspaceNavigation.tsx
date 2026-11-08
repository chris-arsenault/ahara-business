import { useEffect, useRef, useState } from "react";
import {
  Activity,
  CalendarDays,
  FolderLock,
  Forward,
  LogOut,
  Mail,
  Menu,
  ReceiptText,
  Route,
  ShieldCheck,
  Users,
  X,
  type LucideIcon,
} from "lucide-react";
import { AppMark } from "./appMark";
import type { ActiveView } from "./appWorkspace";
import "./workspaceNavigation.css";

type NavItem = { icon: LucideIcon; label: string; view: ActiveView };
const work: NavItem[] = [
  { icon: Mail, label: "Mailbox", view: "mailbox" },
  { icon: Users, label: "Contacts", view: "contacts" },
  { icon: CalendarDays, label: "Calendar", view: "calendar" },
  { icon: FolderLock, label: "Files", view: "files" },
  { icon: ReceiptText, label: "Finance", view: "finance" },
];
const manage: NavItem[] = [
  { icon: Route, label: "Routing", view: "routing" },
  { icon: Forward, label: "Forwarding", view: "forwarding" },
  { icon: ShieldCheck, label: "Authorizations", view: "authorizations" },
  { icon: Activity, label: "Operations", view: "operations" },
];

function useNavigationMenu() {
  const [open, setOpen] = useState(false);
  const menu = useRef<HTMLButtonElement>(null);
  useEffect(() => {
    function escape(event: KeyboardEvent) {
      if (event.key === "Escape" && open) {
        setOpen(false);
        menu.current?.focus();
      }
    }
    window.addEventListener("keydown", escape);
    return () => window.removeEventListener("keydown", escape);
  }, [open]);
  return { open, setOpen, menu };
}

export function WorkspaceNavigation({
  active,
  onSelect,
  productName,
  account,
  onLogout,
}: {
  active: ActiveView;
  onSelect: (view: ActiveView) => void;
  productName: string;
  account: string;
  onLogout: () => void;
}) {
  const { open, setOpen, menu } = useNavigationMenu();
  function select(view: ActiveView) {
    onSelect(view);
    setOpen(false);
  }
  return (
    <aside
      className="app-sidebar"
      aria-label="Application navigation"
      data-open={open}
    >
      <div className="brand-lockup">
        <AppMark className="brand-mark" />
        <span>{productName}</span>
        <button
          ref={menu}
          className="icon-button navigation-toggle"
          type="button"
          aria-label={open ? "Close navigation" : "Open navigation"}
          aria-expanded={open}
          aria-controls="workspace-navigation"
          onClick={() => setOpen((value) => !value)}
        >
          {open ? <X size={20} /> : <Menu size={20} />}
        </button>
      </div>
      <nav id="workspace-navigation" className="app-nav">
        <NavigationGroup
          title="Workspace"
          items={work}
          active={active}
          select={select}
        />
        <NavigationGroup
          title="Manage"
          items={manage}
          active={active}
          select={select}
        />
      </nav>
      <div className="account-strip">
        <span title={account}>{account}</span>
        <button
          className="icon-button"
          type="button"
          aria-label="Sign out"
          title="Sign out"
          onClick={onLogout}
        >
          <LogOut size={17} aria-hidden="true" />
        </button>
      </div>
    </aside>
  );
}

function NavigationGroup({
  title,
  items,
  active,
  select,
}: {
  title: string;
  items: NavItem[];
  active: ActiveView;
  select: (view: ActiveView) => void;
}) {
  return (
    <div className="navigation-group">
      <h2>{title}</h2>
      {items.map((item) => {
        const Icon = item.icon;
        return (
          <button
            key={item.view}
            className="nav-button"
            data-active={active === item.view}
            aria-current={active === item.view ? "page" : undefined}
            type="button"
            onClick={() => select(item.view)}
          >
            <Icon size={17} aria-hidden="true" />
            {item.label}
          </button>
        );
      })}
    </div>
  );
}
