import { useEffect, useRef, useState } from 'react';
import { tauriApi } from '../api/tauri';

export interface WsEvent {
  type: 'BrowserStatusChanged' | 'ActionLogEntry' | 'ProfilesChanged' | 'RecordingPlaybackProgress' | 'Heartbeat';
  data?: any;
}

export interface BrowserStatusEvent {
  profile_id: string;
  running: boolean;
}

export interface ActionLogEntryEvent {
  id: string;
  ts: number;
  profile_id: string;
  tool: string;
  duration_ms: number;
  success: boolean;
  error?: string;
}

export interface RecordingPlaybackProgressEvent {
  recording_id: string;
  profile_id: string;
  action_index: number;
  total_actions: number;
  action_type: string;
  status: 'running' | 'failed' | 'completed';
  error?: string;
}

interface UseWebSocketOptions {
  onBrowserStatus?: (event: BrowserStatusEvent) => void;
  onActionLog?: (entry: ActionLogEntryEvent) => void;
  onProfilesChanged?: () => void;
  onRecordingPlaybackProgress?: (event: RecordingPlaybackProgressEvent) => void;
  onConnect?: () => void;
  onDisconnect?: () => void;
}

/**
 * WebSocket hook for real-time browser events.
 * Automatically reconnects on disconnect and handles connection errors.
 */
export const useWebSocket = (options: UseWebSocketOptions = {}) => {
  const [connected, setConnected] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const wsRef = useRef<WebSocket | null>(null);
  const reconnectTimeoutRef = useRef<number | null>(null);
  const heartbeatTimeoutRef = useRef<number | null>(null);

  const connect = async () => {
    try {
      const config = await tauriApi.getLocalApiConfig();
      if (!config?.enabled) {
        setError('API server not enabled');
        return;
      }

      const wsUrl = config.api_key
        ? `ws://127.0.0.1:${config.api_port}/api/ws?api_key=${encodeURIComponent(config.api_key)}`
        : `ws://127.0.0.1:${config.api_port}/api/ws`;

      const ws = new WebSocket(wsUrl);
      wsRef.current = ws;

      ws.onopen = () => {
        setConnected(true);
        setError(null);
        options.onConnect?.();

        // Start heartbeat check (30s timeout)
        if (heartbeatTimeoutRef.current) {
          clearTimeout(heartbeatTimeoutRef.current);
        }
        heartbeatTimeoutRef.current = setTimeout(() => {
          if (ws.readyState === WebSocket.OPEN) {
            ws.close();
            setConnected(false);
          }
        }, 35000) as unknown as number; // 30s + 5s buffer
      };

      ws.onmessage = (event) => {
        // Reset heartbeat on any message
        if (heartbeatTimeoutRef.current) {
          clearTimeout(heartbeatTimeoutRef.current);
        }
        heartbeatTimeoutRef.current = setTimeout(() => {
          if (ws.readyState === WebSocket.OPEN) {
            ws.close();
            setConnected(false);
          }
        }, 35000) as unknown as number;

        try {
          const wsEvent: WsEvent = JSON.parse(event.data);

          switch (wsEvent.type) {
            case 'BrowserStatusChanged':
              options.onBrowserStatus?.(wsEvent.data as BrowserStatusEvent);
              break;
            case 'ActionLogEntry':
              options.onActionLog?.(wsEvent.data as ActionLogEntryEvent);
              break;
            case 'ProfilesChanged':
              options.onProfilesChanged?.();
              break;
            case 'RecordingPlaybackProgress':
              options.onRecordingPlaybackProgress?.(wsEvent.data as RecordingPlaybackProgressEvent);
              break;
            case 'Heartbeat':
              // Just reset heartbeat timer
              break;
          }
        } catch (e) {
          console.error('Failed to parse WebSocket message:', e);
        }
      };

      ws.onerror = (err) => {
        console.error('WebSocket error:', err);
        setError('Connection error');
      };

      ws.onclose = () => {
        setConnected(false);
        options.onDisconnect?.();

        // Auto-reconnect after 3s
        if (reconnectTimeoutRef.current) {
          clearTimeout(reconnectTimeoutRef.current);
        }
        reconnectTimeoutRef.current = setTimeout(() => {
          connect();
        }, 3000) as unknown as number;
      };
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
      // Retry after 5s
      if (reconnectTimeoutRef.current) {
        clearTimeout(reconnectTimeoutRef.current);
      }
      reconnectTimeoutRef.current = setTimeout(() => {
        connect();
      }, 5000) as unknown as number;
    }
  };

  useEffect(() => {
    connect();

    return () => {
      if (reconnectTimeoutRef.current) {
        clearTimeout(reconnectTimeoutRef.current);
      }
      if (heartbeatTimeoutRef.current) {
        clearTimeout(heartbeatTimeoutRef.current);
      }
      if (wsRef.current) {
        wsRef.current.close();
      }
    };
  }, []);

  return { connected, error };
};
