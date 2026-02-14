import { useState, useEffect } from 'react';
import { tauriApi } from '../api/tauri';
import type { BrowserProfile } from '../types/profile';
import type { ScheduledTask, ScheduleConfig } from '../types/agent';

interface SchedulePanelProps {
  profiles: BrowserProfile[];
}

const dayNames = ['Monday', 'Tuesday', 'Wednesday', 'Thursday', 'Friday', 'Saturday', 'Sunday'];

function formatSchedule(schedule: ScheduleConfig): string {
  switch (schedule.type) {
    case 'once':
      return `Once at ${new Date(schedule.datetime! * 1000).toLocaleString()}`;
    case 'interval':
      return `Every ${schedule.minutes} minute${schedule.minutes! > 1 ? 's' : ''}`;
    case 'daily':
      return `Daily at ${String(schedule.hour!).padStart(2, '0')}:${String(schedule.minute!).padStart(2, '0')}`;
    case 'weekly':
      return `Every ${dayNames[schedule.day_of_week!]} at ${String(schedule.hour!).padStart(2, '0')}:${String(schedule.minute!).padStart(2, '0')}`;
    case 'cron':
      return `Cron: ${schedule.expression}`;
    default:
      return 'Unknown schedule';
  }
}

export function SchedulePanel({ profiles }: SchedulePanelProps) {
  const [tasks, setTasks] = useState<ScheduledTask[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [showForm, setShowForm] = useState(false);

  // Form state
  const [name, setName] = useState('');
  const [task, setTask] = useState('');
  const [selectedProfiles, setSelectedProfiles] = useState<Set<string>>(new Set());
  const [scheduleType, setScheduleType] = useState<ScheduleConfig['type']>('daily');
  const [scheduleHour, setScheduleHour] = useState(9);
  const [scheduleMinute, setScheduleMinute] = useState(0);
  const [scheduleDayOfWeek, setScheduleDayOfWeek] = useState(0);
  const [scheduleInterval, setScheduleInterval] = useState(60);
  const [headless, setHeadless] = useState(true);

  useEffect(() => {
    loadTasks();
  }, []);

  const loadTasks = async () => {
    setLoading(true);
    try {
      const loadedTasks = await tauriApi.getScheduledTasks();
      setTasks(loadedTasks);
    } catch (e) {
      setError(`Failed to load scheduled tasks: ${e}`);
    } finally {
      setLoading(false);
    }
  };

  const handleCreateTask = async () => {
    if (!name.trim() || !task.trim() || selectedProfiles.size === 0) {
      setError('Please fill in all required fields');
      return;
    }

    const schedule: ScheduleConfig = (() => {
      switch (scheduleType) {
        case 'daily':
          return { type: 'daily', hour: scheduleHour, minute: scheduleMinute };
        case 'weekly':
          return { type: 'weekly', day_of_week: scheduleDayOfWeek, hour: scheduleHour, minute: scheduleMinute };
        case 'interval':
          return { type: 'interval', minutes: scheduleInterval };
        default:
          return { type: 'daily', hour: scheduleHour, minute: scheduleMinute };
      }
    })();

    const newTask: ScheduledTask = {
      id: crypto.randomUUID(),
      name,
      task,
      profile_ids: Array.from(selectedProfiles),
      schedule,
      enabled: true,
      headless,
      created_at: Math.floor(Date.now() / 1000),
      run_count: 0,
    };

    try {
      await tauriApi.addScheduledTask(newTask);
      setTasks([...tasks, newTask]);
      resetForm();
      setShowForm(false);
    } catch (e) {
      setError(`Failed to create scheduled task: ${e}`);
    }
  };

  const handleToggleTask = async (taskId: string, enabled: boolean) => {
    try {
      await tauriApi.toggleScheduledTask(taskId, enabled);
      setTasks(tasks.map(t => t.id === taskId ? { ...t, enabled } : t));
    } catch (e) {
      setError(`Failed to update task: ${e}`);
    }
  };

  const handleDeleteTask = async (taskId: string) => {
    try {
      await tauriApi.deleteScheduledTask(taskId);
      setTasks(tasks.filter(t => t.id !== taskId));
    } catch (e) {
      setError(`Failed to delete task: ${e}`);
    }
  };

  const resetForm = () => {
    setName('');
    setTask('');
    setSelectedProfiles(new Set());
    setScheduleType('daily');
    setScheduleHour(9);
    setScheduleMinute(0);
    setScheduleDayOfWeek(0);
    setScheduleInterval(60);
    setHeadless(true);
    setError(null);
  };

  if (loading) {
    return <div className="loading">Loading scheduled tasks...</div>;
  }

  return (
    <div className="schedule-panel">
      <div className="panel-header">
        <h3>Scheduled Tasks</h3>
        <button className="btn btn-primary" onClick={() => setShowForm(!showForm)}>
          {showForm ? 'Cancel' : 'New Schedule'}
        </button>
      </div>

      {error && <div className="error-message">{error}</div>}

      {/* Create Form */}
      {showForm && (
        <div className="schedule-form">
          <h4>Create Scheduled Task</h4>

          <div className="form-group">
            <label>Task Name *</label>
            <input
              type="text"
              value={name}
              onChange={(e) => setName(e.target.value)}
              placeholder="e.g., Daily Order Check"
            />
          </div>

          <div className="form-group">
            <label>Task Description *</label>
            <textarea
              value={task}
              onChange={(e) => setTask(e.target.value)}
              placeholder="e.g., Check pending orders and export to CSV"
              rows={3}
            />
          </div>

          <div className="form-group">
            <label>Select Profiles ({selectedProfiles.size} selected) *</label>
            <div className="profile-checkbox-list">
              <button
                type="button"
                className="btn btn-sm btn-link"
                onClick={() => {
                  if (selectedProfiles.size === profiles.length) {
                    setSelectedProfiles(new Set());
                  } else {
                    setSelectedProfiles(new Set(profiles.map(p => p.id)));
                  }
                }}
              >
                {selectedProfiles.size === profiles.length ? 'Deselect All' : 'Select All'}
              </button>
              {profiles.map((profile) => (
                <label key={profile.id} className="profile-checkbox-item">
                  <input
                    type="checkbox"
                    checked={selectedProfiles.has(profile.id)}
                    onChange={(e) => {
                      const newSet = new Set(selectedProfiles);
                      if (e.target.checked) {
                        newSet.add(profile.id);
                      } else {
                        newSet.delete(profile.id);
                      }
                      setSelectedProfiles(newSet);
                    }}
                  />
                  <span className="profile-name">{profile.name}</span>
                </label>
              ))}
            </div>
          </div>

          <div className="form-group">
            <label>Schedule Type</label>
            <select value={scheduleType} onChange={(e) => setScheduleType(e.target.value as ScheduleConfig['type'])}>
              <option value="daily">Daily</option>
              <option value="weekly">Weekly</option>
              <option value="interval">Interval</option>
            </select>
          </div>

          {scheduleType === 'daily' && (
            <div className="form-row">
              <div className="form-group">
                <label>Hour</label>
                <input
                  type="number"
                  value={scheduleHour}
                  onChange={(e) => setScheduleHour(parseInt(e.target.value) || 0)}
                  min={0}
                  max={23}
                />
              </div>
              <div className="form-group">
                <label>Minute</label>
                <input
                  type="number"
                  value={scheduleMinute}
                  onChange={(e) => setScheduleMinute(parseInt(e.target.value) || 0)}
                  min={0}
                  max={59}
                />
              </div>
            </div>
          )}

          {scheduleType === 'weekly' && (
            <>
              <div className="form-group">
                <label>Day of Week</label>
                <select value={scheduleDayOfWeek} onChange={(e) => setScheduleDayOfWeek(parseInt(e.target.value))}>
                  {dayNames.map((day, i) => (
                    <option key={i} value={i}>{day}</option>
                  ))}
                </select>
              </div>
              <div className="form-row">
                <div className="form-group">
                  <label>Hour</label>
                  <input
                    type="number"
                    value={scheduleHour}
                    onChange={(e) => setScheduleHour(parseInt(e.target.value) || 0)}
                    min={0}
                    max={23}
                  />
                </div>
                <div className="form-group">
                  <label>Minute</label>
                  <input
                    type="number"
                    value={scheduleMinute}
                    onChange={(e) => setScheduleMinute(parseInt(e.target.value) || 0)}
                    min={0}
                    max={59}
                  />
                </div>
              </div>
            </>
          )}

          {scheduleType === 'interval' && (
            <div className="form-group">
              <label>Interval (minutes)</label>
              <input
                type="number"
                value={scheduleInterval}
                onChange={(e) => setScheduleInterval(parseInt(e.target.value) || 60)}
                min={1}
              />
            </div>
          )}

          <div className="form-group">
            <label>
              <input
                type="checkbox"
                checked={headless}
                onChange={(e) => setHeadless(e.target.checked)}
              />
              Headless Mode
            </label>
          </div>

          <div className="form-actions">
            <button className="btn btn-secondary" onClick={() => { resetForm(); setShowForm(false); }}>
              Cancel
            </button>
            <button className="btn btn-primary" onClick={handleCreateTask}>
              Create Schedule
            </button>
          </div>
        </div>
      )}

      {/* Task List */}
      <div className="scheduled-tasks-list">
        {tasks.length === 0 ? (
          <div className="no-tasks">
            <p>No scheduled tasks yet. Click "New Schedule" to create one.</p>
            <p className="hint">Note: Scheduled tasks run when the app is open.</p>
          </div>
        ) : (
          tasks.map((task) => (
            <div key={task.id} className={`scheduled-task-item ${!task.enabled ? 'disabled' : ''}`}>
              <div className="task-info">
                <div className="task-header">
                  <span className="task-name">{task.name}</span>
                  <span className={`status-badge ${task.enabled ? 'active' : 'inactive'}`}>
                    {task.enabled ? 'Active' : 'Paused'}
                  </span>
                </div>
                <div className="task-schedule">{formatSchedule(task.schedule)}</div>
                <div className="task-profiles">
                  {task.profile_ids.length} profile{task.profile_ids.length !== 1 ? 's' : ''}
                </div>
                {task.next_run && (
                  <div className="task-next-run">
                    Next run: {new Date(task.next_run * 1000).toLocaleString()}
                  </div>
                )}
                {task.last_run && (
                  <div className="task-last-run">
                    Last run: {new Date(task.last_run * 1000).toLocaleString()} ({task.run_count} runs total)
                  </div>
                )}
              </div>
              <div className="task-actions">
                <button
                  className={`btn btn-sm ${task.enabled ? 'btn-secondary' : 'btn-primary'}`}
                  onClick={() => handleToggleTask(task.id, !task.enabled)}
                >
                  {task.enabled ? 'Pause' : 'Enable'}
                </button>
                <button
                  className="btn btn-sm btn-danger"
                  onClick={() => handleDeleteTask(task.id)}
                >
                  Delete
                </button>
              </div>
            </div>
          ))
        )}
      </div>
    </div>
  );
}
