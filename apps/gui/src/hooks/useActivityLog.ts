import { useEffect, useRef, useState } from 'react';

export function useActivityLog() {
  const [status, setStatus] = useState('idle');
  const [logs, setLogs] = useState<string[]>([]);
  const logEndRef = useRef<HTMLDivElement | null>(null);

  // Update scrolling logs
  useEffect(() => {
    if (status && status !== 'idle') {
      let displayStatus = status;
      if (status === 'done') {
        displayStatus = 'Bake finished';
      } else if (status === 'source loaded') {
        displayStatus = 'Model loaded successfully';
      }
      const timestamp = new Date().toLocaleTimeString(undefined, {
        hour: '2-digit',
        minute: '2-digit',
        second: '2-digit',
        hour12: true
      });
      setLogs((prev) => [...prev, `[${timestamp}] ${displayStatus}`]);
    }
  }, [status]);

  useEffect(() => {
    logEndRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, [logs]);

  return { status, setStatus, logs, logEndRef };
}
