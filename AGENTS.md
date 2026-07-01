You are working on a project called "augmented-gaussian"

The orginal topic was:
"""
Đề tài: Nghiên cứu, phát triển công cụ xử lý dữ liệu 3D Gaussian Splatting phục vụ hệ thống Thực tế tăng cường (AR)

Mô tả: Là một công cụ tự động xử lý và chuyển đổi dữ liệu 3D Gaussian Splatting (3DGS) thành các mô hình 3D có cấu trúc hình học. Mục tiêu là giúp các vật thể ảo trong môi trường AR có thể nhận biết được vật cản và nhận diện được địa hình trong thực tế.

Yêu cầu chức năng:

 - Tiền xử lý dữ liệu: Đọc file dữ liệu 3DGS đầu vào (định dạng .ply hoặc .splat). Nhận diện và lọc bỏ các điểm ảnh bị nhiễu, mờ hoặc thừa thãi trong không gian quét.

 - Tái tạo và tối ưu hóa hình học: Chuyển đổi đám mây điểm 3DGS thành bề mặt lưới 3D (Mesh). Tối ưu hóa và làm nhẹ lưới 3D này để giảm thiểu dung lượng tải.

 - Trích xuất dữ liệu phục vụ AR: Tạo lưới che khuất (Occlusion Mesh) làm vật cản và lưới điều hướng (Navigation Mesh) làm bản đồ tìm đường.

 - Xuất dữ liệu: Xuất file mô hình kết quả cuối cùng dưới định dạng 3D chuẩn phổ biến, tối ưu cho môi trường web/AR (ví dụ: .glb).

Yêu cầu phi chức năng:

 - Triển khai: Đóng gói thành công cụ chạy tự động qua dòng lệnh (CLI) hoặc có giao diện người dùng đơn giản, dễ thao tác.

 - Chất lượng đầu ra: Đảm bảo mô hình xuất ra khớp chính xác về tỷ lệ và tọa độ so với thế giới thực, đồng thời đủ nhẹ để tải mượt mà trên ứng dụng. 





