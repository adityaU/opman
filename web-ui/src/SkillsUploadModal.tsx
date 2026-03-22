import React, { useState, useRef } from "react";
import { X, Upload, FileText } from "lucide-react";

interface SkillsUploadModalProps {
  onClose: () => void;
}

export function SkillsUploadModal({ onClose }: SkillsUploadModalProps) {
  const [uploading, setUploading] = useState(false);
  const [message, setMessage] = useState<string | null>(null);
  const fileInputRef = useRef<HTMLInputElement>(null);

  const handleFileUpload = async (event: React.ChangeEvent<HTMLInputElement>) => {
    const file = event.target.files?.[0];
    if (!file) return;

    if (!file.name.toLowerCase().endsWith('.zip')) {
      setMessage("Please select a ZIP file");
      return;
    }

    setUploading(true);
    setMessage(null);

    try {
      const formData = new FormData();
      formData.append('skills_zip', file);

      const response = await fetch('/api/skills/upload', {
        method: 'POST',
        credentials: 'same-origin',
        body: formData,
      });

      if (response.ok) {
        setMessage("Skills uploaded successfully!");
        // Close after a short delay
        setTimeout(onClose, 1500);
      } else {
        setMessage("Upload failed. Please try again.");
      }
    } catch (error) {
      console.error('Upload failed:', error);
      setMessage("Upload failed. Please try again.");
    } finally {
      setUploading(false);
    }
  };

  return (
    <div className="skills-upload-drawer">
      <div className="drawer-content">
        <div className="drawer-header">
          <h2>Upload Skills</h2>
          <button className="drawer-close" onClick={onClose}>
            <X size={20} />
          </button>
        </div>

        <div className="drawer-body">
          <div className="upload-area">
            <div className="upload-instructions">
              <FileText size={48} className="upload-icon" />
              <p>Upload a ZIP file containing skills folders.</p>
              <p>Each skill should be in its own folder with a SKILL.md file.</p>
            </div>

            <input
              ref={fileInputRef}
              type="file"
              accept=".zip"
              onChange={handleFileUpload}
              style={{ display: 'none' }}
            />

            <button
              className="upload-button"
              onClick={() => fileInputRef.current?.click()}
              disabled={uploading}
            >
              <Upload size={16} />
              {uploading ? 'Uploading...' : 'Select ZIP File'}
            </button>

            {message && (
              <div className={`upload-message ${message.includes('success') ? 'success' : 'error'}`}>
                {message}
              </div>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}