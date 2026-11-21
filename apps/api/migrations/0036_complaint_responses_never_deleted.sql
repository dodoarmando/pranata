-- A posted Complaint Response stays on the status page for good.
CREATE TRIGGER complaint_responses_never_deleted BEFORE DELETE ON complaint_responses
    FOR EACH ROW EXECUTE FUNCTION reject_change();
