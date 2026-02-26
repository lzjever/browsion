import React from 'react';

interface ConfirmDialogProps {
  message: string;
  onConfirm: () => void;
  onCancel: () => void;
  confirmLabel?: string;
  confirmClassName?: string;
}

export const ConfirmDialog: React.FC<ConfirmDialogProps> = ({
  message,
  onConfirm,
  onCancel,
  confirmLabel = 'Confirm',
  confirmClassName = 'btn btn-danger',
}) => (
  <div className="modal-overlay">
    <div className="confirm-dialog">
      <p className="confirm-message">{message}</p>
      <div className="confirm-actions">
        <button className="btn btn-secondary" onClick={onCancel}>Cancel</button>
        <button className={confirmClassName} onClick={onConfirm}>{confirmLabel}</button>
      </div>
    </div>
  </div>
);
