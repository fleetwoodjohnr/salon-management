import "react-big-calendar/lib/css/react-big-calendar.css";
import "react-big-calendar/lib/addons/dragAndDrop/styles.css";
import { Badge, Button, Drawer, Group, SegmentedControl, Select, Stack, Text, Title } from "@mantine/core";
import { useHotkeys } from "@mantine/hooks";
import { modals } from "@mantine/modals";
import { notifications } from "@mantine/notifications";
import { IconChevronLeft, IconChevronRight, IconPlus } from "@tabler/icons-react";
import { useQueryClient } from "@tanstack/react-query";
import dayjs from "dayjs";
import { useMemo, useState } from "react";
import { Calendar, dayjsLocalizer, type View } from "react-big-calendar";
import withDragAndDropModule from "react-big-calendar/lib/addons/dragAndDrop";
import { useNavigate } from "react-router";
import { call, errorMessage } from "../../api/ipc";
import { useCmd } from "../../api/queries";
import type { AppointmentInput, AppointmentSaveResult, AppointmentView, Business, Staff } from "../../api/types";
import { PageHeader } from "../../components/PageHeader";
import { fromLocalStr, timeLabel, toLocalStr } from "../../lib/time";
import { notifyUndo } from "../../components/undo";
import { AppointmentModal } from "./AppointmentModal";

const localizer = dayjsLocalizer(dayjs);
// The addon is CommonJS; depending on the bundler its function is the default export or `.default` on it.
const withDragAndDrop = ((withDragAndDropModule as unknown as { default?: typeof withDragAndDropModule }).default ?? withDragAndDropModule) as typeof withDragAndDropModule;
// eslint-disable-next-line @typescript-eslint/no-explicit-any
const DnDCalendar = withDragAndDrop<CalEvent, Resource>(Calendar as any);

interface CalEvent {
  id: number;
  title: string;
  start: Date;
  end: Date;
  resourceId: number;
  appt: AppointmentView;
}
interface Resource {
  id: number;
  title: string;
}

const statusLabels: Record<string, string> = { scheduled: "Booked", checked_in: "Checked in", completed: "Completed", cancelled: "Cancelled", no_show: "No-show" };

function rangeFor(date: Date, view: View): [string, string] {
  const d = dayjs(date);
  const from = view === "week" ? d.startOf("week") : d.startOf("day");
  const to = view === "week" ? from.add(7, "day") : from.add(1, "day");
  return [toLocalStr(from.toDate()), toLocalStr(to.toDate())];
}

export function CalendarPage() {
  const qc = useQueryClient();
  const navigate = useNavigate();
  const [date, setDate] = useState(new Date());
  const [view, setView] = useState<View>("day");
  const [staffFilter, setStaffFilter] = useState<string | null>(null);
  const [editing, setEditing] = useState<AppointmentInput | null>(null);
  const [selected, setSelected] = useState<AppointmentView | null>(null);
  const staff = useCmd<Staff[]>("staff_list");
  const biz = useCmd<Business>("business_get");
  const [from, to] = rangeFor(date, view);
  const appts = useCmd<AppointmentView[]>("appointments_list", { from, to, staffId: staffFilter ? Number(staffFilter) : null, clientId: null });
  const activeStaff = (staff.data ?? []).filter((s) => !s.archived && (!staffFilter || String(s.id) === staffFilter));

  const events: CalEvent[] = useMemo(
    () =>
      (appts.data ?? []).map((a) => ({
        id: a.id,
        title: `${a.client_name || "Walk-in"} — ${a.service_names.join(", ")}`,
        start: fromLocalStr(a.starts_at),
        end: fromLocalStr(a.ends_at),
        resourceId: a.staff_id,
        appt: a,
      })),
    [appts.data],
  );
  const open = biz.data?.schedule.filter((d) => d.open) ?? [];
  const minH = open.length ? Math.max(0, Math.min(...open.map((d) => Number(d.start.slice(0, 2)))) - 1) : 7;
  const maxH = open.length ? Math.min(24, Math.max(...open.map((d) => Number(d.end.slice(0, 2)) + (d.end.slice(3) === "00" ? 0 : 1))) + 1) : 21;

  function newAt(start: Date, staffId?: number) {
    const sid = staffId ?? activeStaff[0]?.id;
    if (!sid) {
      notifications.show({ message: "Add a staff member in Settings first.", color: "yellow" });
      return;
    }
    setEditing({ id: null, client_id: null, staff_id: sid, location_id: null, starts_at: toLocalStr(start), ends_at: null, notes: "", lines: [{ service_id: 0, variant_ids: [], qty: 1 }], estimate_id: null, allow_overlap: false });
  }
  useHotkeys([["mod+N", () => newAt(dayjs().add(1, "hour").startOf("hour").toDate())]]);

  async function move(ev: CalEvent, start: Date, end: Date, resourceId?: number, allow = false) {
    try {
      const r = await call<AppointmentSaveResult>("appointment_reschedule", { id: ev.id, startsAt: toLocalStr(start), endsAt: toLocalStr(end), staffId: resourceId ?? ev.resourceId, allowOverlap: allow });
      if (!r.id) {
        modals.openConfirmModal({
          title: "This overlaps another booking",
          children: <Text size="sm">{r.conflicts.join("; ")}</Text>,
          labels: { confirm: "Move anyway", cancel: "Keep where it was" },
          onConfirm: () => move(ev, start, end, resourceId, true),
        });
        return;
      }
      r.warnings.forEach((w) => notifications.show({ message: w, color: "blue" }));
      notifications.show({ message: `Moved to ${start.toLocaleString(undefined, { weekday: "short", hour: "numeric", minute: "2-digit" })}`, color: "teal" });
      qc.invalidateQueries({ queryKey: ["appointments_list"] });
    } catch (e) {
      notifications.show({ title: "Couldn't move it", message: errorMessage(e), color: "red" });
    }
  }

  async function setStatus(a: AppointmentView, status: string, undoable = true) {
    try {
      await call("appointment_status", { id: a.id, status });
      qc.invalidateQueries({ queryKey: ["appointments_list"] });
      setSelected(null);
      if (undoable) notifyUndo(`Marked ${statusLabels[status].toLowerCase()}`, () => setStatus(a, a.status, false));
    } catch (e) {
      notifications.show({ title: "Couldn't update", message: errorMessage(e), color: "red" });
    }
  }

  async function checkout(a: AppointmentView) {
    try {
      const saleId = await call<number>("sale_from_appointment", { appointmentId: a.id });
      qc.invalidateQueries({ queryKey: ["appointments_list"] });
      navigate(`/sales/${saleId}`);
    } catch (e) {
      notifications.show({ title: "Couldn't start checkout", message: errorMessage(e), color: "red" });
    }
  }

  const step = view === "week" ? 7 : 1;
  return (
    <>
      <PageHeader
        title="Calendar"
        description="Drag to reschedule. Click an empty slot to book. Overlapping bookings for the same person are flagged before they're saved."
        actions={
          <Button leftSection={<IconPlus size={16} />} onClick={() => newAt(dayjs(date).hour(10).minute(0).toDate())}>
            New appointment
          </Button>
        }
      />
      <Group justify="space-between" mb="sm">
        <Group gap="xs">
          <Button variant="default" size="xs" onClick={() => setDate(new Date())}>
            Today
          </Button>
          <Button variant="default" size="xs" px={6} aria-label="Previous" onClick={() => setDate(dayjs(date).subtract(step, "day").toDate())}>
            <IconChevronLeft size={16} />
          </Button>
          <Button variant="default" size="xs" px={6} aria-label="Next" onClick={() => setDate(dayjs(date).add(step, "day").toDate())}>
            <IconChevronRight size={16} />
          </Button>
          <Title order={4} ml="sm">
            {view === "week" ? `Week of ${dayjs(date).startOf("week").format("MMM D, YYYY")}` : dayjs(date).format("dddd, MMMM D, YYYY")}
          </Title>
        </Group>
        <Group gap="xs">
          <Select
            size="xs"
            aria-label="Staff"
            placeholder="Everyone"
            clearable
            w={160}
            data={(staff.data ?? []).filter((s) => !s.archived).map((s) => ({ value: String(s.id), label: s.name }))}
            value={staffFilter}
            onChange={setStaffFilter}
          />
          <SegmentedControl
            size="xs"
            value={view}
            onChange={(v) => setView(v as View)}
            data={[
              { value: "day", label: "Day" },
              { value: "week", label: "Week" },
            ]}
          />
        </Group>
      </Group>
      <div className="srm-calendar">
        <DnDCalendar
          localizer={localizer}
          events={events}
          view={view}
          onView={setView}
          date={date}
          onNavigate={setDate}
          toolbar={false}
          step={15}
          timeslots={4}
          min={dayjs(date).hour(minH).minute(0).toDate()}
          max={dayjs(date).hour(Math.min(maxH, 23)).minute(59).toDate()}
          resources={view === "day" && activeStaff.length > 1 ? activeStaff.map((s) => ({ id: s.id!, title: s.name })) : undefined}
          resourceIdAccessor={(r: Resource) => r.id}
          resourceTitleAccessor={(r: Resource) => r.title}
          selectable
          resizable
          popup
          onSelectSlot={(slot) => newAt(slot.start as Date, (slot as { resourceId?: number }).resourceId)}
          onSelectEvent={(e) => setSelected(e.appt)}
          onEventDrop={({ event, start, end, resourceId }) => move(event, start as Date, end as Date, resourceId as number | undefined)}
          onEventResize={({ event, start, end }) => move(event, start as Date, end as Date)}
          draggableAccessor={(e) => e.appt.status === "scheduled" || e.appt.status === "checked_in"}
          eventPropGetter={(e) => ({
            className: `srm-event srm-event-${e.appt.status}`,
            style: { ["--srm-ev" as string]: `var(--mantine-color-${e.appt.staff_color}-6)` },
          })}
          style={{ height: "calc(100vh - 250px)", minHeight: 520 }}
        />
      </div>
      {editing && (
        <AppointmentModal
          initial={editing}
          onClose={() => setEditing(null)}
          onSaved={(r) => {
            r.warnings.forEach((w) => notifications.show({ message: w, color: "blue" }));
            notifications.show({ message: "Appointment saved", color: "teal" });
            setEditing(null);
          }}
        />
      )}
      <Drawer opened={!!selected} onClose={() => setSelected(null)} position="right" title="Appointment" size="md">
        {selected && (
          <Stack>
            <Group gap="xs">
              <Title order={4}>{selected.client_name || "Walk-in"}</Title>
              <Badge color={selected.status === "cancelled" || selected.status === "no_show" ? "gray" : selected.status === "completed" ? "teal" : "mulberry"}>
                {statusLabels[selected.status]}
              </Badge>
            </Group>
            <Text size="sm">
              {fromLocalStr(selected.starts_at).toLocaleDateString(undefined, { weekday: "long", month: "long", day: "numeric" })}, {timeLabel(selected.starts_at)}–{timeLabel(selected.ends_at)} with{" "}
              {selected.staff_name}
            </Text>
            <Text size="sm">{selected.service_names.join(", ")}</Text>
            {selected.notes && (
              <Text size="sm" c="dimmed">
                {selected.notes}
              </Text>
            )}
            <Group gap="xs" mt="md">
              {selected.sale_status === "finalized" ? (
                <Button onClick={() => navigate(`/sales/${selected.sale_id}`)}>View sale</Button>
              ) : selected.status !== "cancelled" && selected.status !== "no_show" ? (
                <Button onClick={() => checkout(selected)}>{selected.sale_id ? "Continue checkout" : "Complete and check out"}</Button>
              ) : null}
              {selected.status === "scheduled" && (
                <Button variant="default" onClick={() => setStatus(selected, "checked_in")}>
                  Check in
                </Button>
              )}
              {(selected.status === "scheduled" || selected.status === "checked_in") && !selected.sale_id && (
                <Button
                  variant="default"
                  onClick={() => {
                    setEditing({ ...selected, ends_at: null, estimate_id: null, allow_overlap: false });
                    setSelected(null);
                  }}
                >
                  Edit
                </Button>
              )}
            </Group>
            {(selected.status === "scheduled" || selected.status === "checked_in") && !selected.sale_id && (
              <Group gap="xs">
                <Button size="xs" variant="subtle" color="gray" onClick={() => setStatus(selected, "cancelled")}>
                  Cancel appointment
                </Button>
                <Button size="xs" variant="subtle" color="gray" onClick={() => setStatus(selected, "no_show")}>
                  Mark no-show
                </Button>
              </Group>
            )}
            {(selected.status === "cancelled" || selected.status === "no_show") && (
              <Button size="xs" variant="subtle" w="fit-content" onClick={() => setStatus(selected, "scheduled")}>
                Restore booking
              </Button>
            )}
          </Stack>
        )}
      </Drawer>
    </>
  );
}
