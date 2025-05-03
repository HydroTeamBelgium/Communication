from tkinter import filedialog


# Change to the directory you want to choose your image from. Or you can just change to /

filename = filedialog.askopenfilename(initialdir = "/",
                                          title = "Select a File",
                                          filetypes = (("Images",
                                                        "*.jpg*"),
                                                       ("all files",
                                                        "*.*")))

# print(filename)

with open(filename, 'rb') as image_file:
    byte_data = image_file.read()

# Transform byte_data(hexadecimal format?) into a list of u8 integers
u8_array = list(byte_data)
# print(u8_array)
print(f"Loaded {len(u8_array)} bytes")

# writing to file
new_text_file = open('demo/myfile.txt', 'w')
new_text_file.write("[")
for byte in u8_array[:-1]:
    new_text_file.write(f"{byte}, ")
new_text_file.write(f"{u8_array[-1]}")

new_text_file.write("]")
new_text_file.close()