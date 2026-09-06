// Index-level contract: a plain list of projects with search, an opt-in sort, the
// empty-state recovery action, and archived kept out of the way.
//
// What this file no longer asserts, because the surface no longer does it: the
// "needs a decision" grouping, review-reason badges, and the review-interval line. The
// list reports the projects; it does not rank them.

import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter } from "react-router-dom";
import { describe, expect, it, vi } from "vitest";

import { projectId } from "../../domain/project";
import { indexItem } from "../../test/fixtures";
import { ProjectsIndex } from "./ProjectsIndex";

const NOW = new Date("2026-08-12T12:00:00Z");

function renderIndex(
  projects = [indexItem()],
  opts: { url?: string; onAddProject?: () => void } = {},
) {
  return render(
    <MemoryRouter initialEntries={[opts.url ?? "/projects"]}>
      <ProjectsIndex
        projects={projects}
        now={NOW}
        onAddProject={opts.onAddProject ?? (() => {})}
      />
    </MemoryRouter>,
  );
}

// The row link's accessible name is a composed summary; the stable visible name is .op-row__name.
function linkOrder(): string[] {
  return screen
    .getAllByRole("link")
    .map((l) => l.querySelector(".op-row__name")?.textContent ?? "");
}

describe("the list itself", () => {
  it("is a plain semantic list with no table chrome and no ranking control", () => {
    const { container } = renderIndex();
    expect(screen.getByRole("list", { name: "Projects" })).toBeInTheDocument();
    expect(container.querySelector(".op-index__head")).not.toBeInTheDocument();
    expect(screen.queryByRole("combobox", { name: /priority|review order/i })).not.toBeInTheDocument();
  });

  it("shows how much of each project is left, without opening it", () => {
    renderIndex([indexItem({ name: "Atlas", open_steps: 3, total_steps: 7 })]);
    expect(screen.getByText("3 of 7 steps left")).toBeInTheDocument();
  });

  it("carries none of the review vocabulary the redesign removed", () => {
    renderIndex();
    for (const gone of [/needs your decision/i, /other projects/i, /needs review/i, /review interval/i]) {
      expect(screen.queryByText(gone)).not.toBeInTheDocument();
    }
  });
});

describe("sort", () => {
  const projects = [
    indexItem({ project_id: projectId("c"), name: "Charlie" }),
    indexItem({ project_id: projectId("a"), name: "Alpha" }),
    indexItem({ project_id: projectId("b"), name: "Bravo" }),
  ];

  it("sorts by name when opted in", () => {
    renderIndex(projects, { url: "/projects?sort=name" });
    expect(linkOrder()).toEqual(["Alpha", "Bravo", "Charlie"]);
  });

  it("sorts by how much work is left when opted in", () => {
    renderIndex(
      [
        indexItem({ project_id: projectId("few"), name: "Few", open_steps: 1 }),
        indexItem({ project_id: projectId("many"), name: "Many", open_steps: 9 }),
      ],
      { url: "/projects?sort=remaining" },
    );
    expect(linkOrder()).toEqual(["Many", "Few"]);
  });
});

describe("filters", () => {
  it("filters by name from the q search param", () => {
    renderIndex(
      [indexItem({ name: "Atlas" }), indexItem({ project_id: projectId("z"), name: "Zephyr" })],
      { url: "/projects?q=zep" },
    );
    expect(linkOrder()).toEqual(["Zephyr"]);
  });

});

describe("empty and archived", () => {
  it("offers a focusable Add project action when there are no projects", async () => {
    const user = userEvent.setup();
    const onAddProject = vi.fn();
    renderIndex([], { onAddProject });
    const button = screen.getByRole("button", { name: /add project/i });
    button.focus();
    expect(button).toHaveFocus();
    await user.click(button);
    expect(onAddProject).toHaveBeenCalledTimes(1);
  });

  it("keeps archived projects out of the operating view but exposes an Archived filter", async () => {
    const user = userEvent.setup();
    renderIndex([
      indexItem({ project_id: projectId("live"), name: "Live", status: "active" }),
      indexItem({ project_id: projectId("gone"), name: "Gone", status: "archived" }),
    ]);
    expect(linkOrder()).toEqual(["Live"]);
    await user.click(screen.getByRole("checkbox", { name: "Archived" }));
    expect(linkOrder()).toEqual(["Gone"]);
  });
});
