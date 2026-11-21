import type { ComplaintState } from './api/generated';

/** How staff name a Complaint's state; the public status page words New for the sender. */
export const complaintStateLabels: Record<ComplaintState, string> = {
  new: 'Baru',
  in_progress: 'Diproses',
  closed: 'Selesai',
  rejected: 'Ditolak',
};

export const complaintStateVariants = {
  new: 'default',
  in_progress: 'secondary',
  closed: 'outline',
  rejected: 'destructive',
} as const satisfies Record<ComplaintState, string>;
