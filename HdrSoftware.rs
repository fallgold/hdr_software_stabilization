/*
 * Copyright (C) 2013 The CyanogenMod Project
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *      http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

#pragma version(1)
#pragma rs java_package_name(com.example.bitm)

#include "rs_graphics.rsh"

rs_script gScript;

rs_allocation gInIndex;
uchar4* gInputLow;
uchar4* gInputMid;
uchar4* gInputHi;
uchar4* gOutput;

int gImageWidth;
int gImageHeight;

const int BlockSize = 100;
const int CompMaxOffset = 50;
int BlockLeft = 0;
int BlockTop = 0;
int offsetTopMid = 0, offsetLeftMid = 0;
int offsetTopHi = 0, offsetLeftHi = 0;

void init() {
	rsDebug("HDR init", rsUptimeMillis());
}

void root(const int32_t* v_in, int32_t* v_out) {
	// Get the row from the input
	int32_t y = *v_in;
	int32_t y1 = y / gImageWidth;

	// Compute the average of each pixels from the 3 image samples
	float3 pxOut;
	
	for (int x = 0; x < gImageWidth; x++) {
		const int32_t index = y+x;
		float3 pxLow, pxMid, pxHi;

		// Gather the pixels
		pxLow = rsUnpackColor8888(gInputLow[index]).rgb;
		pxMid = pxLow;
		if (!offsetTopMid || (y1 > -offsetTopMid && (y1 + offsetTopMid) < gImageHeight)) {
			int32_t indexMid = index + gImageWidth * offsetTopMid;
			if (!offsetLeftMid) {
				pxMid = rsUnpackColor8888(gInputMid[indexMid]).rgb;
			} else if (x > -offsetLeftMid && (x + offsetLeftMid) < gImageWidth) {
				pxMid = rsUnpackColor8888(gInputMid[indexMid + offsetLeftMid]).rgb;
			}
		}
		pxHi = pxLow;
		if (!offsetTopHi || (y1 > -offsetTopHi && (y1 + offsetTopHi) < gImageHeight)) {
			int32_t indexHi = index + gImageWidth * offsetTopHi;
			if (!offsetLeftHi) {
				pxHi = rsUnpackColor8888(gInputHi[indexHi]).rgb;
			} else if (x > -offsetLeftHi && (x + offsetLeftHi) < gImageWidth) {
				pxHi = rsUnpackColor8888(gInputHi[indexHi + offsetLeftHi]).rgb;
			}
		}

		// Compute the average
		pxOut.r = (pxLow.r + pxMid.r + pxHi.r) / 3.0f;
		pxOut.g = (pxLow.g + pxMid.g + pxHi.g) / 3.0f;
		pxOut.b = (pxLow.b + pxMid.b + pxHi.b) / 3.0f;

		// Copy the pixel to the output image
		gOutput[index] = rsPackColorTo8888(pxOut);
	}
}

static void findBlock(uchar4 *inputP, int* blockTop, int* blockLeft) {
	int x0, y0, maxX;
	if (gImageWidth > gImageHeight) {
		x0 = (gImageWidth - gImageHeight) / 2;
		maxX = gImageWidth - x0 - 1; // make sure not out of bounds
		y0 = 0;
	} else {
		x0 = 0;
		maxX = gImageWidth;
		y0 = (gImageHeight - gImageWidth) / 2;
	}

	const int maxDiffCount = BlockSize - 1;
	float diffRing[maxDiffCount] = {0};
	int diffIndex = 0;
	float curDiff = 0, maxDiff = 0;
	float preGray = 0;
	int blockIndex = y0 * gImageWidth + x0;
	for(int x = x0, y = y0; x < maxX; x++, y++, blockIndex += (gImageWidth + 1)) {
		float3 px = rsUnpackColor8888(inputP[blockIndex]).rgb;
		float gray = px.r * 0.299f + px.g * 0.587f + px.b * 0.114f;
		float diff = fabs(gray - preGray);
		float headDiff = diffRing[diffIndex];
		curDiff -= headDiff;
		curDiff += diff;
		if (curDiff > maxDiff) {
			*blockTop = y;
			*blockLeft = x;
			maxDiff = curDiff;
		}
		diffRing[diffIndex] = diff;
		if (++diffIndex >= maxDiffCount)
			diffIndex = 0;
		preGray = gray;
	}
	rsDebug("find block : ", *blockTop, *blockLeft, maxDiff);
}

static void findOffset(uchar4 *inputP, uchar4 *inputQ, int* offsetTop, int* offsetLeft, float* maxSim) {
	const int BB = BlockSize * BlockSize;
	const int BOO = BlockSize + CompMaxOffset * 2;
	const int BOO2 = BOO * BOO;
	float grayP[BB], grayQ[BOO2];
	float avgGrayP = 0, avgGrayQ = 0;
	
	for (int i = 0; i < BOO2; i++) 
		grayQ[i] = -1;
	*maxSim = -1;

	int blockIndex = BlockTop * gImageWidth + BlockLeft;
	int grayIndex = 0;
	avgGrayP = 0;
	for(int m = 0; m < BlockSize; m++, blockIndex += (gImageWidth - BlockSize)) {
		for(int n = 0; n < BlockSize; n++, blockIndex++) {
			float3 px = rsUnpackColor8888(inputP[blockIndex]).rgb;
			float gray = px.r * 0.299f + px.g * 0.587f + px.b * 0.114f;
			grayP[grayIndex++] = gray;
			avgGrayP += gray;
		}
	}
	avgGrayP /= BB;

	int fullOffset = CompMaxOffset * 2;
	int firstBlockIndex = (BlockTop > CompMaxOffset ? (BlockTop - CompMaxOffset) : 0) * gImageWidth 
				+ (BlockLeft > CompMaxOffset ? (BlockLeft - CompMaxOffset) : 0);
	for(int i = 0; i <= fullOffset; i++, firstBlockIndex += (gImageWidth - (fullOffset + 1))) {
		for(int j = 0; j <= fullOffset; j++, firstBlockIndex++) {
			blockIndex = firstBlockIndex;
			avgGrayQ = 0;
			for(int m = 0; m < BlockSize; m++, blockIndex += (gImageWidth - BlockSize)) {
				grayIndex = (i + m) * BOO + j;
				for(int n = 0; n < BlockSize; n++, blockIndex++, grayIndex++) {
					float gray;
					if (grayQ[grayIndex] < 0) {
						float3 px = rsUnpackColor8888(inputQ[blockIndex]).rgb;
						gray = px.r * 0.299f + px.g * 0.587f + px.b * 0.114f;
						grayQ[grayIndex] = gray;
						avgGrayQ += gray;
					} else {
						gray = grayQ[grayIndex];
						avgGrayQ += grayQ[grayIndex];
					}
				}
			}
			avgGrayQ /= BB;

			float num = 0;
			float denPP = 0, denQQ = 0;
			int grayIndexP = 0;
			for(int m = 0; m < BlockSize; m++) {
				int grayIndexQ = (i + m) * BOO + j;
				for(int n = 0; n < BlockSize; n++, grayIndexP++, grayIndexQ++) {
					float diffP = grayP[grayIndexP] - avgGrayP;
					float diffQ = grayQ[grayIndexQ] - avgGrayQ;
					num += fabs(diffP * diffQ);
					denPP += diffP * diffP;
					denQQ += diffQ * diffQ;
				}
			}
			float sim = num / sqrt(denPP * denQQ);
			if(sim > *maxSim) {
				*maxSim = sim;
				*offsetTop = i - (BlockTop > CompMaxOffset ? CompMaxOffset : BlockTop);
				*offsetLeft = j - (BlockLeft > CompMaxOffset ? CompMaxOffset : BlockLeft);
			}
		}
	}
}

void performHdrComputation() {
	if (gInputLow == 0 || gInputMid == 0
		|| gInputHi == 0 || gOutput == 0) {
		// TODO: Compute if there are only 2 images
		rsDebug("There are pointers missing, skipping rendering.", rsUptimeMillis());

	} else {
		findBlock(gInputLow, &BlockTop, &BlockLeft);

		float maxSim;
		findOffset(gInputLow, gInputMid, &offsetTopMid, &offsetLeftMid, &maxSim);
		rsDebug("offset Mid: ", maxSim, offsetTopMid, offsetLeftMid);
		findOffset(gInputLow, gInputHi, &offsetTopHi, &offsetLeftHi, &maxSim);
		rsDebug("offset Hi: ", maxSim, offsetTopHi, offsetLeftHi);

		// v_out is not used, so we pass gInIndex again
		rsForEach(gScript, gInIndex, gInIndex, 0);
	}
}
